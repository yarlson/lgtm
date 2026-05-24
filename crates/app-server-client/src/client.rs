use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::{
    config::AppServerConfig,
    json::{get_string, response_result},
    transcript::{CompletedTurn, TurnCollector},
};

const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

pub struct AppServerClient {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    config: AppServerConfig,
}

impl AppServerClient {
    pub fn connect(config: AppServerConfig) -> Result<Self> {
        let mut client = Self::spawn(config)?;
        client.initialize()?;
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

        Ok(Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
            next_id: 1,
            config,
        })
    }

    fn initialize(&mut self) -> Result<()> {
        let request = json!({
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": self.config.client_name.as_str(),
                    "title": self.config.client_title.as_str(),
                    "version": self.config.client_version.as_str()
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }
        });
        self.call(request).context("initialize failed")?;
        self.notify(json!({
            "method": "initialized",
            "params": {}
        }))
        .context("failed to send initialized notification")
    }

    pub fn start_thread(&mut self) -> Result<String> {
        let cwd = std::env::current_dir()
            .context("failed to read current directory")?
            .display()
            .to_string();

        let result = self
            .call(json!({
                "method": "thread/start",
                "params": {
                    "approvalPolicy": self.config.approval_policy.as_str(),
                    "cwd": cwd,
                    "developerInstructions": self.config.developer_instructions.as_str(),
                    "ephemeral": true,
                    "model": self.config.model.as_str(),
                    "sandbox": self.config.sandbox.as_str(),
                    "serviceName": self.config.service_name.as_str()
                }
            }))
            .context("thread/start failed")?;

        get_string(&result, &["thread", "id"])
            .context("thread/start response did not include thread.id")
    }

    pub fn run_turn(&mut self, thread_id: &str, prompt: &str) -> Result<CompletedTurn> {
        let result = self
            .call(json!({
                "method": "turn/start",
                "params": {
                    "threadId": thread_id,
                    "effort": self.config.reasoning_effort.as_str(),
                    "input": [{ "type": "text", "text": prompt }]
                }
            }))
            .with_context(|| format!("turn/start failed for prompt `{prompt}`"))?;

        let turn_id = get_string(&result, &["turn", "id"])
            .context("turn/start response did not include turn.id")?;
        self.wait_for_turn(thread_id, &turn_id)
            .with_context(|| format!("turn {turn_id} did not complete cleanly"))
    }

    fn call(&mut self, mut request: Value) -> Result<Value> {
        let id = self.next_request_id();
        request
            .as_object_mut()
            .context("request must be a JSON object")?
            .insert("id".to_string(), json!(id));

        self.write_json(&request)?;

        loop {
            let message = self.read_json()?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                return response_result(message);
            }

            self.handle_message(message)?;
        }
    }

    fn notify(&mut self, notification: Value) -> Result<()> {
        self.write_json(&notification)
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
        if message.get("id").is_some() && message.get("method").is_some() {
            self.decline_server_request(&message)?;
        }

        Ok(())
    }

    fn decline_server_request(&mut self, request: &Value) -> Result<()> {
        let id = request
            .get("id")
            .cloned()
            .context("server request had no id")?;
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");

        let response = match method {
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                json!({ "id": id, "result": { "decision": "decline" } })
            }
            _ => json!({
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Unsupported server request: {method}")
                }
            }),
        };

        self.write_json(&response)
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

    pub fn stop(mut self) -> Result<()> {
        drop(self.stdin.take());

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

impl Drop for AppServerClient {
    fn drop(&mut self) {
        let _ = self.stdin.take();
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
