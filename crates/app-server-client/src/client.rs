use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::{
    config::AppServerConfig,
    json::{get_string, response_result},
    protocol::{ClientNotification, ClientRequest, ServerRequest},
    transcript::{CompletedTurn, TurnCollector},
};

const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

pub struct AppServerClient {
    child: Child,
    connection: AppServerConnection<BufReader<ChildStdout>, ChildStdin>,
}

impl AppServerClient {
    pub fn connect(config: AppServerConfig) -> Result<Self> {
        let mut client = Self::spawn(config)?;
        client.connection.initialize()?;
        Ok(client)
    }

    fn spawn(config: AppServerConfig) -> Result<Self> {
        let mut child = Command::new("codex")
            .args(["app-server"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("failed to start `codex app-server`")?;

        let stdin = child
            .stdin
            .take()
            .context("codex app-server stdin was unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("codex app-server stdout was unavailable")?;

        let connection = AppServerConnection::new(config, BufReader::new(stdout), stdin);

        Ok(Self { child, connection })
    }

    pub fn start_thread(&mut self) -> Result<String> {
        self.connection.start_thread()
    }

    pub fn run_turn(&mut self, thread_id: &str, prompt: &str) -> Result<CompletedTurn> {
        self.connection.run_turn(thread_id, prompt)
    }

    pub fn stop(mut self) -> Result<()> {
        drop(self.connection.stdin.take());

        let deadline = Instant::now() + SHUTDOWN_GRACE;
        while Instant::now() < deadline {
            if let Some(status) = self
                .child
                .try_wait()
                .context("failed to poll codex app-server process")?
            {
                if status.success() {
                    return Ok(());
                }
                bail!("codex app-server exited with {status}");
            }

            thread::sleep(Duration::from_millis(50));
        }

        self.child
            .kill()
            .context("failed to stop codex app-server after closing stdin")?;
        self.child
            .wait()
            .context("failed to wait for killed codex app-server")?;
        Ok(())
    }
}

struct AppServerConnection<R, W> {
    stdin: Option<W>,
    stdout: R,
    next_id: u64,
    config: AppServerConfig,
}

impl<R, W> AppServerConnection<R, W>
where
    R: BufRead,
    W: Write,
{
    fn new(config: AppServerConfig, stdout: R, stdin: W) -> Self {
        Self {
            stdin: Some(stdin),
            stdout,
            next_id: 1,
            config,
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let config = self.config.clone();
        self.call(ClientRequest::Initialize { config: &config })
            .context("initialize failed")?;
        self.notify(ClientNotification::Initialized)
            .context("failed to send initialized notification")
    }

    pub fn start_thread(&mut self) -> Result<String> {
        let config = self.config.clone();
        let cwd = std::env::current_dir()
            .context("failed to read current directory")?
            .display()
            .to_string();

        let result = self
            .call(ClientRequest::ThreadStart {
                config: &config,
                cwd,
            })
            .context("thread/start failed")?;

        get_string(&result, &["thread", "id"])
            .context("thread/start response did not include thread.id")
    }

    pub fn run_turn(&mut self, thread_id: &str, prompt: &str) -> Result<CompletedTurn> {
        let effort = self.config.reasoning_effort.clone();
        let result = self
            .call(ClientRequest::TurnStart {
                thread_id,
                prompt,
                effort: effort.as_str(),
            })
            .with_context(|| format!("turn/start failed for prompt `{prompt}`"))?;

        let turn_id = get_string(&result, &["turn", "id"])
            .context("turn/start response did not include turn.id")?;
        self.wait_for_turn(thread_id, &turn_id)
            .with_context(|| format!("turn {turn_id} did not complete cleanly"))
    }

    fn call(&mut self, request: ClientRequest<'_>) -> Result<Value> {
        let id = self.next_request_id();
        let request = request.into_message(id);
        self.write_json(&request)?;

        loop {
            let message = self.read_json()?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                return response_result(message);
            }

            self.handle_message(message)?;
        }
    }

    fn notify(&mut self, notification: ClientNotification) -> Result<()> {
        self.write_json(&notification.into_message())
    }

    fn wait_for_turn(&mut self, thread_id: &str, turn_id: &str) -> Result<CompletedTurn> {
        let mut collector = TurnCollector::new(thread_id, turn_id);

        loop {
            let message = self.read_json()?;
            if let Some(turn) = collector.handle(&message)? {
                return Ok(turn);
            }

            self.handle_message(message)?;
        }
    }

    fn handle_message(&mut self, message: Value) -> Result<()> {
        if let Some(request) = ServerRequest::from_message(&message) {
            self.write_json(&request.decline_response())?;
        }

        Ok(())
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn write_json(&mut self, value: &Value) -> Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .context("codex app-server stdin is closed")?;
        serde_json::to_writer(&mut *stdin, value)
            .context("failed to serialize JSON-RPC message")?;
        stdin
            .write_all(b"\n")
            .context("failed to write JSON-RPC newline")?;
        stdin
            .flush()
            .context("failed to flush codex app-server stdin")
    }

    fn read_json(&mut self) -> Result<Value> {
        let mut line = String::new();
        let bytes = self
            .stdout
            .read_line(&mut line)
            .context("failed to read codex app-server stdout")?;
        if bytes == 0 {
            bail!("codex app-server exited before sending the expected message");
        }

        serde_json::from_str(line.trim_end())
            .with_context(|| format!("codex app-server emitted invalid JSON: {}", line.trim_end()))
    }
}

impl Drop for AppServerClient {
    fn drop(&mut self) {
        let _ = self.connection.stdin.take();
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use serde_json::{Value, json};

    use super::*;

    fn test_config() -> AppServerConfig {
        AppServerConfig {
            model: "gpt-5.5".to_string(),
            reasoning_effort: "high".to_string(),
            sandbox: "danger-full-access".to_string(),
            approval_policy: "never".to_string(),
            developer_instructions: "Reply naturally.".to_string(),
            service_name: "lgtm-rs".to_string(),
            client_name: "lgtm-rs".to_string(),
            client_title: "lgtm-rs".to_string(),
            client_version: "0.1.0".to_string(),
        }
    }

    fn connection(messages: Vec<Value>) -> AppServerConnection<Cursor<Vec<u8>>, Vec<u8>> {
        let input = messages
            .into_iter()
            .map(|message| serde_json::to_string(&message).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        AppServerConnection::new(test_config(), Cursor::new(input.into_bytes()), Vec::new())
    }

    fn written_messages(connection: &AppServerConnection<Cursor<Vec<u8>>, Vec<u8>>) -> Vec<Value> {
        let output = std::str::from_utf8(connection.stdin.as_ref().unwrap()).unwrap();
        output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn initialize_handles_server_requests_before_response() {
        let mut connection = connection(vec![
            json!({
                "id": 99,
                "method": "item/tool/requestUserInput",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "questions": []
                }
            }),
            json!({
                "id": 1,
                "result": {
                    "userAgent": "test",
                    "codexHome": "/tmp/codex"
                }
            }),
        ]);

        connection.initialize().unwrap();

        let written = written_messages(&connection);
        assert_eq!(written.len(), 3);
        assert_eq!(written[0]["method"], "initialize");
        assert_eq!(written[0]["id"], 1);
        assert_eq!(written[0]["params"]["clientInfo"]["name"], "lgtm-rs");
        assert_eq!(
            written[1],
            json!({
                "id": 99,
                "result": {
                    "answers": {}
                }
            })
        );
        assert_eq!(written[2]["method"], "initialized");
    }

    #[test]
    fn run_turn_declines_approval_requests_while_collecting_transcript() {
        let mut connection = connection(vec![
            json!({
                "id": 1,
                "result": {
                    "turn": {
                        "id": "turn_456",
                        "status": "inProgress",
                        "items": []
                    }
                }
            }),
            json!({
                "method": "item/agentMessage/delta",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "itemId": "msg_1",
                    "delta": "Done."
                }
            }),
            json!({
                "id": 22,
                "method": "item/commandExecution/requestApproval",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "itemId": "cmd_1",
                    "command": "cargo test"
                }
            }),
            json!({
                "method": "turn/completed",
                "params": {
                    "threadId": "thr_123",
                    "turn": {
                        "id": "turn_456",
                        "items": [],
                        "status": "completed"
                    }
                }
            }),
        ]);

        let turn = connection.run_turn("thr_123", "Run tests").unwrap();

        assert_eq!(turn.turn_id, "turn_456");
        assert_eq!(turn.status, "completed");
        assert_eq!(turn.transcript.response_text(), "Done.");

        let written = written_messages(&connection);
        assert_eq!(written.len(), 2);
        assert_eq!(written[0]["method"], "turn/start");
        assert_eq!(written[0]["params"]["threadId"], "thr_123");
        assert_eq!(written[0]["params"]["effort"], "high");
        assert_eq!(
            written[0]["params"]["input"],
            json!([{ "type": "text", "text": "Run tests" }])
        );
        assert_eq!(
            written[1],
            json!({
                "id": 22,
                "result": {
                    "decision": "decline"
                }
            })
        );
    }

    #[test]
    fn unknown_server_requests_are_rejected_explicitly() {
        let mut connection = connection(vec![
            json!({
                "id": 41,
                "method": "future/request",
                "params": {}
            }),
            json!({
                "id": 1,
                "result": {
                    "thread": {
                        "id": "thr_123"
                    }
                }
            }),
        ]);

        let thread_id = connection.start_thread().unwrap();

        assert_eq!(thread_id, "thr_123");
        let written = written_messages(&connection);
        assert_eq!(written.len(), 2);
        assert_eq!(written[0]["method"], "thread/start");
        assert_eq!(
            written[1],
            json!({
                "id": 41,
                "error": {
                    "code": -32601,
                    "message": "Unsupported server request: future/request"
                }
            })
        );
    }
}
