use std::{
    io::Write,
    process::{Child, ChildStdin, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::app_server::{
    config::AppServerConfig,
    json::{get_string, response_result},
    launch::AppServerLaunch,
    line_source::{LineRead, LineSource, ThreadedLineReader},
    protocol::{ClientNotification, ClientRequest, ServerRequest},
    transcript::{CompletedTurn, PlanStep, TranscriptEvent, TranscriptItem, TurnCollector},
};

const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
const STREAM_IDLE_TICK: Duration = Duration::from_millis(120);

type RawMessageSink = Box<dyn FnMut(&str) -> Result<()> + Send>;

pub struct AppServerClient {
    child: Child,
    connection: AppServerConnection<ThreadedLineReader, ChildStdin>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnControl {
    Continue,
    Interrupt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnStreamEvent {
    Idle,
    PlanUpdated(Vec<PlanStep>),
    ItemUpdated(TranscriptItem),
    ServerRequestDeclined { method: String },
    Completed(CompletedTurn),
}

impl From<TranscriptEvent> for TurnStreamEvent {
    fn from(event: TranscriptEvent) -> Self {
        match event {
            TranscriptEvent::PlanUpdated(plan) => Self::PlanUpdated(plan),
            TranscriptEvent::ItemUpdated(item) => Self::ItemUpdated(item),
            TranscriptEvent::Completed(turn) => Self::Completed(turn),
        }
    }
}

impl AppServerClient {
    pub fn connect(launch: AppServerLaunch, config: AppServerConfig) -> Result<Self> {
        let mut client = Self::spawn(launch, config)?;
        client.connection.initialize()?;
        Ok(client)
    }

    fn spawn(launch: AppServerLaunch, config: AppServerConfig) -> Result<Self> {
        let mut command = Command::new(launch.program());
        command
            .args(launch.args())
            .envs(launch.envs().iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to start app-server command: {}",
                launch.display_command()
            )
        })?;

        let stdin = child
            .stdin
            .take()
            .context("codex app-server stdin was unavailable")?;
        let stdout = child
            .stdout
            .take()
            .context("codex app-server stdout was unavailable")?;

        let connection = AppServerConnection::new(config, ThreadedLineReader::spawn(stdout), stdin);

        Ok(Self { child, connection })
    }

    pub fn start_thread(&mut self) -> Result<String> {
        self.connection.start_thread()
    }

    pub fn log_raw_messages(&mut self, sink: impl FnMut(&str) -> Result<()> + Send + 'static) {
        self.connection.raw_message_sink = Some(Box::new(sink));
    }

    pub fn run_turn_streaming(
        &mut self,
        thread_id: &str,
        prompt: &str,
        on_event: impl FnMut(TurnStreamEvent) -> TurnControl,
    ) -> Result<CompletedTurn> {
        self.connection
            .run_turn_streaming(thread_id, prompt, on_event)
    }

    pub fn run_turn_streaming_with_effort(
        &mut self,
        thread_id: &str,
        prompt: &str,
        effort: &str,
        on_event: impl FnMut(TurnStreamEvent) -> TurnControl,
    ) -> Result<CompletedTurn> {
        self.connection
            .run_turn_streaming_with_effort(thread_id, prompt, effort, on_event)
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
    raw_message_sink: Option<RawMessageSink>,
}

impl<R, W> AppServerConnection<R, W>
where
    R: LineSource,
    W: Write,
{
    fn new(config: AppServerConfig, stdout: R, stdin: W) -> Self {
        Self {
            stdin: Some(stdin),
            stdout,
            next_id: 1,
            config,
            raw_message_sink: None,
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

        let result = self
            .call(ClientRequest::ThreadStart {
                config: &config,
                cwd: config.cwd.clone(),
            })
            .context("thread/start failed")?;

        get_string(&result, &["thread", "id"])
            .context("thread/start response did not include thread.id")
    }

    #[cfg(test)]
    pub fn run_turn(&mut self, thread_id: &str, prompt: &str) -> Result<CompletedTurn> {
        self.run_turn_streaming(thread_id, prompt, |_| TurnControl::Continue)
    }

    pub fn run_turn_streaming(
        &mut self,
        thread_id: &str,
        prompt: &str,
        on_event: impl FnMut(TurnStreamEvent) -> TurnControl,
    ) -> Result<CompletedTurn> {
        let effort = self.config.reasoning_effort.clone();
        self.run_turn_streaming_with_effort(thread_id, prompt, effort.as_str(), on_event)
    }

    pub fn run_turn_streaming_with_effort(
        &mut self,
        thread_id: &str,
        prompt: &str,
        effort: &str,
        on_event: impl FnMut(TurnStreamEvent) -> TurnControl,
    ) -> Result<CompletedTurn> {
        let result = self
            .call(ClientRequest::TurnStart {
                thread_id,
                prompt,
                effort,
            })
            .with_context(|| format!("turn/start failed for prompt `{prompt}`"))?;

        let turn_id = get_string(&result, &["turn", "id"])
            .context("turn/start response did not include turn.id")?;
        self.wait_for_turn_streaming(thread_id, &turn_id, on_event)
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

    fn wait_for_turn_streaming(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        mut on_event: impl FnMut(TurnStreamEvent) -> TurnControl,
    ) -> Result<CompletedTurn> {
        let mut collector = TurnCollector::new(thread_id, turn_id);
        let mut interrupt_requested = false;
        let mut pending_interrupt_id = None;
        let mut completed_turn = None;

        loop {
            let Some(message) = self.read_json_timeout(Some(STREAM_IDLE_TICK))? else {
                let _ = on_event(TurnStreamEvent::Idle);
                continue;
            };
            if pending_interrupt_id
                .is_some_and(|id| message.get("id").and_then(Value::as_u64) == Some(id))
            {
                response_result(message).context("turn/interrupt failed")?;
                pending_interrupt_id = None;
                if let Some(turn) = completed_turn {
                    return Ok(turn);
                }
                continue;
            }

            let is_completion_notification =
                message.get("method").and_then(Value::as_str) == Some("turn/completed");
            for event in collector.handle_stream_event(&message)? {
                let event = TurnStreamEvent::from(event);
                let completed = match &event {
                    TurnStreamEvent::Completed(turn) => Some(turn.clone()),
                    _ => None,
                };
                let control = on_event(event);

                if let Some(turn) = completed {
                    if pending_interrupt_id.is_none() {
                        return Ok(turn);
                    }
                    completed_turn = Some(turn);
                    continue;
                }

                if control == TurnControl::Interrupt
                    && !interrupt_requested
                    && !is_completion_notification
                {
                    pending_interrupt_id = Some(self.interrupt_turn(thread_id, turn_id)?);
                    interrupt_requested = true;
                }
            }

            if let Some(event) = self.handle_message(message)?
                && on_event(event) == TurnControl::Interrupt
                && !interrupt_requested
            {
                pending_interrupt_id = Some(self.interrupt_turn(thread_id, turn_id)?);
                interrupt_requested = true;
            }
        }
    }

    fn interrupt_turn(&mut self, thread_id: &str, turn_id: &str) -> Result<u64> {
        let id = self.next_request_id();
        let request = ClientRequest::TurnInterrupt { thread_id, turn_id }.into_message(id);
        self.write_json(&request)?;
        Ok(id)
    }

    fn handle_message(&mut self, message: Value) -> Result<Option<TurnStreamEvent>> {
        if let Some(request) = ServerRequest::from_message(&message) {
            let method = request.method().to_string();
            self.write_json(&request.decline_response())?;
            return Ok(Some(TurnStreamEvent::ServerRequestDeclined { method }));
        }

        Ok(None)
    }

    fn next_request_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn write_json(&mut self, value: &Value) -> Result<()> {
        let line = serde_json::to_string(value).context("failed to serialize JSON-RPC message")?;
        self.log_raw_message("out", &line)?;

        let stdin = self
            .stdin
            .as_mut()
            .context("codex app-server stdin is closed")?;
        stdin
            .write_all(line.as_bytes())
            .context("failed to write JSON-RPC message")?;
        stdin
            .write_all(b"\n")
            .context("failed to write JSON-RPC newline")?;
        stdin
            .flush()
            .context("failed to flush codex app-server stdin")
    }

    fn read_json(&mut self) -> Result<Value> {
        self.read_json_timeout(None)?
            .context("codex app-server read unexpectedly timed out")
    }

    fn read_json_timeout(&mut self, timeout: Option<Duration>) -> Result<Option<Value>> {
        let line = match self
            .stdout
            .read_line(timeout)
            .context("failed to read codex app-server stdout")?
        {
            LineRead::Line(line) => line,
            LineRead::Idle => return Ok(None),
            LineRead::Eof => bail!("codex app-server exited before sending the expected message"),
        };

        self.log_raw_message("in", line.trim_end())?;

        serde_json::from_str(line.trim_end())
            .with_context(|| format!("codex app-server emitted invalid JSON: {}", line.trim_end()))
            .map(Some)
    }

    fn log_raw_message(&mut self, direction: &'static str, line: &str) -> Result<()> {
        if let Some(sink) = self.raw_message_sink.as_mut() {
            let mut entry = serde_json::to_string(&json!({ "direction": direction, "line": line }))
                .context("failed to serialize app-server log entry")?;
            entry.push('\n');
            sink(&entry)?;
        }
        Ok(())
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
    use std::{collections::VecDeque, io::Cursor};

    use serde_json::{Value, json};

    use super::*;

    fn test_config() -> AppServerConfig {
        AppServerConfig {
            cwd: "/repo".to_string(),
            model: Some("gpt-5.5".to_string()),
            reasoning_effort: "high".to_string(),
            sandbox: "danger-full-access".to_string(),
            approval_policy: "never".to_string(),
            developer_instructions: "Reply naturally.".to_string(),
            service_name: "lgtm".to_string(),
            client_name: "lgtm".to_string(),
            client_title: "lgtm".to_string(),
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

    enum ScriptedRead {
        Line(Value),
        Idle,
    }

    struct ScriptedLineSource {
        reads: VecDeque<ScriptedRead>,
    }

    impl ScriptedLineSource {
        fn new(reads: Vec<ScriptedRead>) -> Self {
            Self {
                reads: reads.into(),
            }
        }
    }

    impl LineSource for ScriptedLineSource {
        fn read_line(&mut self, _timeout: Option<Duration>) -> Result<LineRead> {
            match self.reads.pop_front() {
                Some(ScriptedRead::Line(value)) => Ok(LineRead::Line(format!(
                    "{}\n",
                    serde_json::to_string(&value)?
                ))),
                Some(ScriptedRead::Idle) => Ok(LineRead::Idle),
                None => Ok(LineRead::Eof),
            }
        }
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
        assert_eq!(written[0]["params"]["clientInfo"]["name"], "lgtm");
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
    fn run_turn_streaming_accepts_effort_override() {
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

        connection
            .run_turn_streaming_with_effort("thr_123", "Parse", "low", |_| TurnControl::Continue)
            .unwrap();

        let written = written_messages(&connection);
        assert_eq!(written[0]["params"]["effort"], "low");
    }

    #[test]
    fn run_turn_streaming_emits_progress_and_completion_events() {
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
                "method": "turn/plan/updated",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "plan": [
                        { "step": "Run tests", "status": "inProgress" }
                    ]
                }
            }),
            json!({
                "method": "item/commandExecution/outputDelta",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "itemId": "cmd_1",
                    "delta": "running tests\n"
                }
            }),
            json!({
                "id": 77,
                "method": "item/fileChange/requestApproval",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "itemId": "file_1"
                }
            }),
            json!({
                "method": "turn/completed",
                "params": {
                    "threadId": "thr_123",
                    "turn": {
                        "id": "turn_456",
                        "items": [
                            {
                                "type": "commandExecution",
                                "id": "cmd_1",
                                "command": "cargo test",
                                "cwd": "/repo",
                                "commandActions": [],
                                "status": "completed",
                                "aggregatedOutput": "test ok\n",
                                "exitCode": 0,
                                "durationMs": 123
                            }
                        ],
                        "status": "completed"
                    }
                }
            }),
        ]);
        let mut events = Vec::new();

        let turn = connection
            .run_turn_streaming("thr_123", "Run tests", |event| {
                events.push(event);
                TurnControl::Continue
            })
            .unwrap();

        assert_eq!(turn.status, "completed");
        assert!(matches!(
            &events[0],
            TurnStreamEvent::PlanUpdated(plan)
                if plan.len() == 1 && plan[0].step == "Run tests"
        ));
        assert!(matches!(
            &events[1],
            TurnStreamEvent::ItemUpdated(item)
                if item.title == "command" && item.output() == "running tests\n"
        ));
        assert_eq!(
            events[2],
            TurnStreamEvent::ServerRequestDeclined {
                method: "item/fileChange/requestApproval".to_string()
            }
        );
        assert!(matches!(
            &events[3],
            TurnStreamEvent::ItemUpdated(item)
                if item.title == "$ cargo test" && item.output() == "test ok\n"
        ));
        assert!(matches!(
            events.last(),
            Some(TurnStreamEvent::Completed(completed)) if completed.status == "completed"
        ));

        let written = written_messages(&connection);
        assert_eq!(
            written[1],
            json!({
                "id": 77,
                "result": {
                    "decision": "decline"
                }
            })
        );
    }

    #[test]
    fn run_turn_streaming_emits_idle_events_between_messages() {
        let mut connection = AppServerConnection::new(
            test_config(),
            ScriptedLineSource::new(vec![
                ScriptedRead::Line(json!({
                    "id": 1,
                    "result": {
                        "turn": {
                            "id": "turn_456",
                            "status": "inProgress",
                            "items": []
                        }
                    }
                })),
                ScriptedRead::Idle,
                ScriptedRead::Line(json!({
                    "method": "turn/completed",
                    "params": {
                        "threadId": "thr_123",
                        "turn": {
                            "id": "turn_456",
                            "items": [],
                            "status": "completed"
                        }
                    }
                })),
            ]),
            Vec::new(),
        );
        let mut saw_idle = false;

        connection
            .run_turn_streaming("thr_123", "prompt", |event| {
                if event == TurnStreamEvent::Idle {
                    saw_idle = true;
                }
                TurnControl::Continue
            })
            .expect("turn");

        assert!(saw_idle);
    }

    #[test]
    fn run_turn_streaming_can_interrupt_after_progress_event() {
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
                    "delta": "working"
                }
            }),
            json!({
                "id": 2,
                "result": {}
            }),
            json!({
                "method": "turn/completed",
                "params": {
                    "threadId": "thr_123",
                    "turn": {
                        "id": "turn_456",
                        "items": [],
                        "status": "interrupted"
                    }
                }
            }),
        ]);

        let turn = connection
            .run_turn_streaming("thr_123", "Start", |event| match event {
                TurnStreamEvent::ItemUpdated(_) => TurnControl::Interrupt,
                _ => TurnControl::Continue,
            })
            .unwrap();

        assert_eq!(turn.status, "interrupted");
        let written = written_messages(&connection);
        assert_eq!(written[0]["method"], "turn/start");
        assert_eq!(written[1]["method"], "turn/interrupt");
        assert_eq!(written[1]["id"], 2);
        assert_eq!(written[1]["params"]["threadId"], "thr_123");
        assert_eq!(written[1]["params"]["turnId"], "turn_456");
    }

    #[test]
    fn run_turn_streaming_does_not_interrupt_after_completion_items() {
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
                "method": "turn/completed",
                "params": {
                    "threadId": "thr_123",
                    "turn": {
                        "id": "turn_456",
                        "items": [
                            {
                                "type": "commandExecution",
                                "id": "cmd_1",
                                "command": "cargo test",
                                "cwd": "/repo",
                                "commandActions": [],
                                "status": "completed",
                                "aggregatedOutput": "ok",
                                "exitCode": 0,
                                "durationMs": 123
                            }
                        ],
                        "status": "completed"
                    }
                }
            }),
        ]);

        let turn = connection
            .run_turn_streaming("thr_123", "Start", |event| match event {
                TurnStreamEvent::ItemUpdated(_) => TurnControl::Interrupt,
                _ => TurnControl::Continue,
            })
            .unwrap();

        assert_eq!(turn.status, "completed");
        let written = written_messages(&connection);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0]["method"], "turn/start");
    }

    #[test]
    fn run_turn_streaming_reports_interrupt_errors() {
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
                    "delta": "working"
                }
            }),
            json!({
                "id": 2,
                "error": {
                    "code": -32000,
                    "message": "no active turn"
                }
            }),
        ]);

        let err = connection
            .run_turn_streaming("thr_123", "Start", |event| match event {
                TurnStreamEvent::ItemUpdated(_) => TurnControl::Interrupt,
                _ => TurnControl::Continue,
            })
            .unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("turn/interrupt failed"));
        assert!(message.contains("no active turn"));
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
