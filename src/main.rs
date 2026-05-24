use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use serde_json::{Value, json};

const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
const MODEL: &str = "gpt-5.5";
const REASONING_EFFORT: &str = "high";
const SANDBOX: &str = "danger-full-access";
const APPROVAL_POLICY: &str = "never";
const VALIDATION_PROMPTS: &[&str] = &[
    "Use the shell command tool to run `pwd` and `ls -la` in the current directory. Then summarize what files are present.",
    "Use the shell command tool to run `find . -maxdepth 2 -type f | sort`. Then summarize the project layout.",
    "Use the shell command tool to inspect `Cargo.toml` and report the package name, edition, and dependencies.",
    "Use web search to find current Rust CLI best practices in 2026. Return three concise bullets and include source URLs.",
    "Use web search to find current OpenAI Codex app-server or Codex open-source documentation. Return the most relevant source URL and one sentence.",
    "Use your todo/plan tool to create a three-step plan for validating this CLI's transcript rendering. Then complete the plan in your answer without running commands.",
    "Use your todo/plan tool to plan a tiny refactor of this CLI into transport, transcript, and main modules. Do not edit files; just produce the plan and final recommendation.",
    "Use the shell command tool to run `cargo test`. Summarize pass/fail status.",
    "Use the shell command tool to run `cargo clippy --all-targets -- -D warnings`. Summarize pass/fail status.",
    "Use both a short todo/plan and a shell command: plan two steps, run `cargo fmt -- --check`, then report the result.",
];

#[derive(Debug, Parser)]
#[command(version, about = "Run a simple Codex app-server hello loop")]
struct Cli;

fn main() -> Result<()> {
    Cli::parse();

    let mut server = CodexServer::start()?;
    server.initialize()?;
    let thread_id = server.start_thread()?;

    println!("Codex app-server hello loop");
    println!("Thread: {thread_id}");
    println!("Model: {MODEL}");
    println!("Effort: {REASONING_EFFORT}");
    println!("Mode: yolo ({SANDBOX}, approvals {APPROVAL_POLICY})");
    println!();

    for (index, prompt) in VALIDATION_PROMPTS.iter().enumerate() {
        let n = index + 1;
        let turn = server.run_turn(&thread_id, prompt)?;
        print_turn(n, prompt, &turn);
    }

    server.stop()?;
    Ok(())
}

fn print_turn(n: usize, prompt: &str, turn: &CompletedTurn) {
    println!("Turn {n}");
    println!("  prompt:");
    for line in prompt.lines() {
        println!("    {line}");
    }
    println!("  id: {}", turn.turn_id);
    println!("  status: {}", turn.status);

    if !turn.transcript.plan.is_empty() {
        println!("  plan:");
        for step in &turn.transcript.plan {
            println!("    [{}] {}", step.status, step.step);
        }
    }

    let activity = turn.transcript.activity_items();
    if !activity.is_empty() {
        println!("  activity:");
        for item in activity {
            println!("    - {}", item.title);
            for detail in &item.details {
                println!("      {detail}");
            }
            if let Some(output) = item.output_preview() {
                println!("      output:");
                for line in output.lines() {
                    println!("        {line}");
                }
            }
        }
    }

    let response = turn.transcript.response_text();
    if response.trim().is_empty() {
        println!("  response: <empty>");
    } else {
        println!("  response:");
        for line in response.trim().lines() {
            println!("    {line}");
        }
    }

    println!();
}

struct CodexServer {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl CodexServer {
    fn start() -> Result<Self> {
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
        })
    }

    fn initialize(&mut self) -> Result<()> {
        let request = json!({
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "lgtm-rs",
                    "title": "lgtm-rs",
                    "version": env!("CARGO_PKG_VERSION")
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

    fn start_thread(&mut self) -> Result<String> {
        let cwd = std::env::current_dir()
            .context("failed to read current directory")?
            .display()
            .to_string();

        let result = self
            .call(json!({
                "method": "thread/start",
                "params": {
                    "approvalPolicy": APPROVAL_POLICY,
                    "cwd": cwd,
                    "developerInstructions": "Reply naturally to each user message.",
                    "ephemeral": true,
                    "model": MODEL,
                    "sandbox": SANDBOX,
                    "serviceName": "lgtm-rs"
                }
            }))
            .context("thread/start failed")?;

        get_string(&result, &["thread", "id"])
            .context("thread/start response did not include thread.id")
    }

    fn run_turn(&mut self, thread_id: &str, prompt: &str) -> Result<CompletedTurn> {
        let result = self
            .call(json!({
                "method": "turn/start",
                "params": {
                    "threadId": thread_id,
                    "effort": REASONING_EFFORT,
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

    fn stop(mut self) -> Result<()> {
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

impl Drop for CodexServer {
    fn drop(&mut self) {
        let _ = self.stdin.take();
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CompletedTurn {
    turn_id: String,
    status: String,
    transcript: TurnTranscript,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TurnTranscript {
    plan: Vec<PlanStep>,
    item_order: Vec<String>,
    items: BTreeMap<String, TranscriptItem>,
}

impl TurnTranscript {
    fn upsert_item(&mut self, item: TranscriptItem) {
        if !self.items.contains_key(&item.id) {
            self.item_order.push(item.id.clone());
        }
        self.items.insert(item.id.clone(), item);
    }

    fn append_delta(&mut self, item_id: String, kind: ItemKind, delta: &str) {
        if !self.items.contains_key(&item_id) {
            let title = match kind {
                ItemKind::AgentMessage => "assistant message",
                ItemKind::Plan => "plan",
                ItemKind::Reasoning => "reasoning",
                ItemKind::CommandExecution => "command",
                ItemKind::FileChange => "file changes",
                ItemKind::WebSearch => "web search",
                ItemKind::McpToolCall => "mcp tool",
                ItemKind::DynamicToolCall => "dynamic tool",
                ItemKind::Other => "item",
            };
            self.upsert_item(TranscriptItem::new(item_id.clone(), kind, title));
        }

        if let Some(item) = self.items.get_mut(&item_id) {
            item.output.push_str(delta);
        }
    }

    fn response_text(&self) -> String {
        self.ordered_items()
            .filter(|item| item.kind == ItemKind::AgentMessage)
            .filter_map(|item| item.text.as_deref().or(non_empty(&item.output)))
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn activity_items(&self) -> Vec<&TranscriptItem> {
        self.ordered_items()
            .filter(|item| item.kind != ItemKind::AgentMessage && item.kind != ItemKind::Plan)
            .filter(|item| item.is_renderable())
            .collect()
    }

    fn ordered_items(&self) -> impl Iterator<Item = &TranscriptItem> {
        self.item_order
            .iter()
            .filter_map(|item_id| self.items.get(item_id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlanStep {
    status: String,
    step: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ItemKind {
    AgentMessage,
    Plan,
    Reasoning,
    CommandExecution,
    FileChange,
    WebSearch,
    McpToolCall,
    DynamicToolCall,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TranscriptItem {
    id: String,
    kind: ItemKind,
    title: String,
    details: Vec<String>,
    text: Option<String>,
    output: String,
}

impl TranscriptItem {
    fn new(id: String, kind: ItemKind, title: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            title: title.into(),
            details: Vec::new(),
            text: None,
            output: String::new(),
        }
    }

    fn from_thread_item(item: &Value) -> Option<Self> {
        let id = get_str(item, &["id"]).unwrap_or("<missing-id>").to_string();
        match item.get("type").and_then(Value::as_str)? {
            "userMessage" | "hookPrompt" => None,
            "agentMessage" => {
                let mut transcript_item = Self::new(id, ItemKind::AgentMessage, "assistant");
                transcript_item.text = string_field(item, "text");
                Some(transcript_item)
            }
            "plan" => {
                let mut transcript_item = Self::new(id, ItemKind::Plan, "plan");
                transcript_item.text = string_field(item, "text");
                Some(transcript_item)
            }
            "reasoning" => {
                let mut transcript_item = Self::new(id, ItemKind::Reasoning, "reasoning");
                transcript_item.details = string_array_field(item, "summary")
                    .into_iter()
                    .map(|line| format!("summary: {line}"))
                    .collect();
                let content = string_array_field(item, "content");
                if !content.is_empty() {
                    transcript_item.output = content.join("\n");
                }
                Some(transcript_item)
            }
            "commandExecution" => Some(command_item(id, item)),
            "fileChange" => Some(file_change_item(id, item)),
            "webSearch" => Some(web_search_item(id, item)),
            "mcpToolCall" => Some(mcp_tool_item(id, item)),
            "dynamicToolCall" => Some(dynamic_tool_item(id, item)),
            "collabAgentToolCall" => Some(collab_tool_item(id, item)),
            "imageView" => Some(simple_path_item(id, ItemKind::Other, "image view", item)),
            "enteredReviewMode" => Some(simple_text_item(
                id,
                ItemKind::Other,
                "entered review mode",
                item,
                "review",
            )),
            "exitedReviewMode" => Some(simple_text_item(
                id,
                ItemKind::Other,
                "exited review mode",
                item,
                "review",
            )),
            "contextCompaction" => Some(Self::new(id, ItemKind::Other, "context compaction")),
            other => {
                let mut transcript_item =
                    Self::new(id, ItemKind::Other, format!("unknown item: {other}"));
                transcript_item.output = compact_json(item);
                Some(transcript_item)
            }
        }
    }

    fn output_preview(&self) -> Option<String> {
        let output = non_empty(&self.output)?;
        Some(preview(output, 12, 2_000))
    }

    fn is_renderable(&self) -> bool {
        if matches!(
            self.kind,
            ItemKind::CommandExecution
                | ItemKind::FileChange
                | ItemKind::WebSearch
                | ItemKind::McpToolCall
                | ItemKind::DynamicToolCall
        ) {
            return true;
        }

        !self.details.is_empty()
            || non_empty(&self.output).is_some()
            || self.text.as_deref().and_then(non_empty).is_some()
    }
}

struct TurnCollector<'a> {
    thread_id: &'a str,
    turn_id: &'a str,
    transcript: TurnTranscript,
}

impl<'a> TurnCollector<'a> {
    fn new(thread_id: &'a str, turn_id: &'a str) -> Self {
        Self {
            thread_id,
            turn_id,
            transcript: TurnTranscript::default(),
        }
    }

    fn handle(&mut self, message: &Value) -> Result<Option<CompletedTurn>> {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Ok(None);
        };

        match method {
            "item/agentMessage/delta" => {
                if self.matches_item_turn(message) {
                    self.record_text_delta(message, ItemKind::AgentMessage)?;
                }
            }
            "item/plan/delta" => {
                if self.matches_item_turn(message) {
                    self.record_text_delta(message, ItemKind::Plan)?;
                }
            }
            "item/reasoning/summaryTextDelta" => {
                if self.matches_item_turn(message) {
                    self.record_text_delta(message, ItemKind::Reasoning)?;
                }
            }
            "item/reasoning/textDelta" => {
                if self.matches_item_turn(message) {
                    self.record_text_delta(message, ItemKind::Reasoning)?;
                }
            }
            "item/commandExecution/outputDelta" => {
                if self.matches_item_turn(message) {
                    self.record_text_delta(message, ItemKind::CommandExecution)?;
                }
            }
            "item/fileChange/patchUpdated" => {
                if self.matches_item_turn(message) {
                    self.record_file_patch_update(message)?;
                }
            }
            "item/started" => {
                if self.matches_item_turn(message) {
                    self.record_item(message, &["params", "item"]);
                }
            }
            "item/completed" => {
                if self.matches_item_turn(message) {
                    self.record_item(message, &["params", "item"]);
                }
            }
            "turn/plan/updated" => {
                if self.matches_item_turn(message) {
                    self.record_plan_update(message);
                }
            }
            "turn/completed" => {
                if self.matches_completed_turn(message) {
                    self.record_turn_items(message);
                    return self.completed_turn(message).map(Some);
                }
            }
            "error" => {
                let msg = get_string(message, &["params", "error", "message"])
                    .unwrap_or_else(|_| "unknown Codex error".to_string());
                bail!("codex app-server error: {msg}");
            }
            _ => {}
        }

        Ok(None)
    }

    fn matches_item_turn(&self, message: &Value) -> bool {
        get_str(message, &["params", "threadId"]) == Some(self.thread_id)
            && get_str(message, &["params", "turnId"]) == Some(self.turn_id)
    }

    fn matches_completed_turn(&self, message: &Value) -> bool {
        get_str(message, &["params", "threadId"]) == Some(self.thread_id)
            && get_str(message, &["params", "turn", "id"]) == Some(self.turn_id)
    }

    fn record_text_delta(&mut self, message: &Value, kind: ItemKind) -> Result<()> {
        let item_id = get_string(message, &["params", "itemId"]).context("delta had no itemId")?;
        let delta = get_str(message, &["params", "delta"]).unwrap_or_default();
        self.transcript.append_delta(item_id, kind, delta);
        Ok(())
    }

    fn record_file_patch_update(&mut self, message: &Value) -> Result<()> {
        let item_id =
            get_string(message, &["params", "itemId"]).context("patch update had no itemId")?;
        let mut item = TranscriptItem::new(item_id, ItemKind::FileChange, "file changes");
        item.details = file_change_details(&message["params"]);
        self.transcript.upsert_item(item);
        Ok(())
    }

    fn record_plan_update(&mut self, message: &Value) {
        let Some(plan) = message
            .get("params")
            .and_then(|params| params.get("plan"))
            .and_then(Value::as_array)
        else {
            return;
        };

        self.transcript.plan = plan
            .iter()
            .filter_map(|step| {
                Some(PlanStep {
                    status: get_str(step, &["status"])?.to_string(),
                    step: get_str(step, &["step"])?.to_string(),
                })
            })
            .collect();
    }

    fn record_item(&mut self, message: &Value, path: &[&str]) {
        let Some(item) = get_value(message, path) else {
            return;
        };
        self.record_thread_item(item);
    }

    fn record_turn_items(&mut self, message: &Value) {
        let Some(items) = message
            .get("params")
            .and_then(|params| params.get("turn"))
            .and_then(|turn| turn.get("items"))
            .and_then(Value::as_array)
        else {
            return;
        };

        for item in items {
            self.record_thread_item(item);
        }
    }

    fn record_thread_item(&mut self, item: &Value) {
        if let Some(item) = TranscriptItem::from_thread_item(item) {
            self.transcript.upsert_item(item);
        }
    }

    fn completed_turn(&self, message: &Value) -> Result<CompletedTurn> {
        let status = get_string(message, &["params", "turn", "status"])
            .context("turn/completed did not include turn.status")?;
        let error = get_string(message, &["params", "turn", "error", "message"]).ok();
        if status == "failed" {
            bail!(
                "turn failed: {}",
                error.unwrap_or_else(|| "unknown error".to_string())
            );
        }

        Ok(CompletedTurn {
            turn_id: self.turn_id.to_string(),
            status,
            transcript: self.transcript.clone(),
        })
    }
}

fn response_result(message: Value) -> Result<Value> {
    if let Some(error) = message.get("error") {
        let error_message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown JSON-RPC error");
        return Err(anyhow!("{error_message}"));
    }

    message
        .get("result")
        .cloned()
        .context("JSON-RPC response had neither result nor error")
}

fn get_string(value: &Value, path: &[&str]) -> Result<String> {
    get_str(value, path)
        .map(ToString::to_string)
        .with_context(|| format!("missing string at {}", path.join(".")))
}

fn get_str<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

fn get_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn non_empty(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn command_item(id: String, item: &Value) -> TranscriptItem {
    let command = string_field(item, "command").unwrap_or_else(|| "<unknown command>".to_string());
    let mut transcript_item =
        TranscriptItem::new(id, ItemKind::CommandExecution, format!("$ {command}"));
    push_detail(&mut transcript_item, "cwd", string_field(item, "cwd"));
    push_detail(&mut transcript_item, "status", string_field(item, "status"));
    push_detail(&mut transcript_item, "exit", number_field(item, "exitCode"));
    push_detail(
        &mut transcript_item,
        "duration_ms",
        number_field(item, "durationMs"),
    );
    transcript_item.output = string_field(item, "aggregatedOutput").unwrap_or_default();
    transcript_item
}

fn file_change_item(id: String, item: &Value) -> TranscriptItem {
    let mut transcript_item = TranscriptItem::new(id, ItemKind::FileChange, "file changes");
    push_detail(&mut transcript_item, "status", string_field(item, "status"));
    transcript_item.details.extend(file_change_details(item));
    transcript_item
}

fn file_change_details(value: &Value) -> Vec<String> {
    value
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|change| {
            let path = string_field(change, "path")?;
            let kind = change_kind(change.get("kind"));
            Some(format!("{kind}: {path}"))
        })
        .collect()
}

fn web_search_item(id: String, item: &Value) -> TranscriptItem {
    let query = string_field(item, "query").unwrap_or_else(|| "<empty query>".to_string());
    let mut transcript_item =
        TranscriptItem::new(id, ItemKind::WebSearch, format!("web search: {query}"));
    if let Some(action) = item.get("action").filter(|action| !action.is_null()) {
        transcript_item.details.push(format!(
            "action: {}",
            preview(&compact_json(action), 1, 500)
        ));
    }
    transcript_item
}

fn mcp_tool_item(id: String, item: &Value) -> TranscriptItem {
    let server = string_field(item, "server").unwrap_or_else(|| "<unknown server>".to_string());
    let tool = string_field(item, "tool").unwrap_or_else(|| "<unknown tool>".to_string());
    let mut transcript_item =
        TranscriptItem::new(id, ItemKind::McpToolCall, format!("mcp {server}/{tool}"));
    push_detail(&mut transcript_item, "status", string_field(item, "status"));
    push_detail(
        &mut transcript_item,
        "duration_ms",
        number_field(item, "durationMs"),
    );
    if let Some(arguments) = item.get("arguments") {
        transcript_item.details.push(format!(
            "arguments: {}",
            preview(&compact_json(arguments), 1, 500)
        ));
    }
    if let Some(error) = item.get("error").filter(|error| !error.is_null()) {
        transcript_item
            .details
            .push(format!("error: {}", preview(&compact_json(error), 1, 500)));
    }
    if let Some(result) = item.get("result").filter(|result| !result.is_null()) {
        transcript_item.output = preview(&compact_json(result), 20, 2_000);
    }
    transcript_item
}

fn dynamic_tool_item(id: String, item: &Value) -> TranscriptItem {
    let tool = string_field(item, "tool").unwrap_or_else(|| "<unknown tool>".to_string());
    let title = match string_field(item, "namespace") {
        Some(namespace) => format!("tool {namespace}/{tool}"),
        None => format!("tool {tool}"),
    };
    let mut transcript_item = TranscriptItem::new(id, ItemKind::DynamicToolCall, title);
    push_detail(&mut transcript_item, "status", string_field(item, "status"));
    push_detail(&mut transcript_item, "success", bool_field(item, "success"));
    push_detail(
        &mut transcript_item,
        "duration_ms",
        number_field(item, "durationMs"),
    );
    if let Some(arguments) = item.get("arguments") {
        transcript_item.details.push(format!(
            "arguments: {}",
            preview(&compact_json(arguments), 1, 500)
        ));
    }
    if let Some(content) = item
        .get("contentItems")
        .filter(|content| !content.is_null())
    {
        transcript_item.output = preview(&compact_json(content), 20, 2_000);
    }
    transcript_item
}

fn collab_tool_item(id: String, item: &Value) -> TranscriptItem {
    let tool = string_field(item, "tool").unwrap_or_else(|| "<unknown tool>".to_string());
    let mut transcript_item =
        TranscriptItem::new(id, ItemKind::DynamicToolCall, format!("agent tool {tool}"));
    push_detail(&mut transcript_item, "status", string_field(item, "status"));
    push_detail(&mut transcript_item, "model", string_field(item, "model"));
    push_detail(
        &mut transcript_item,
        "reasoning",
        string_field(item, "reasoningEffort"),
    );
    push_detail(
        &mut transcript_item,
        "sender",
        string_field(item, "senderThreadId"),
    );
    if let Some(prompt) = string_field(item, "prompt") {
        transcript_item
            .details
            .push(format!("prompt: {}", preview(&prompt, 2, 500)));
    }
    transcript_item
}

fn simple_path_item(id: String, kind: ItemKind, title: &str, item: &Value) -> TranscriptItem {
    let mut transcript_item = TranscriptItem::new(id, kind, title);
    push_detail(&mut transcript_item, "path", string_field(item, "path"));
    transcript_item
}

fn simple_text_item(
    id: String,
    kind: ItemKind,
    title: &str,
    item: &Value,
    field: &str,
) -> TranscriptItem {
    let mut transcript_item = TranscriptItem::new(id, kind, title);
    transcript_item.output = string_field(item, field).unwrap_or_default();
    transcript_item
}

fn push_detail(item: &mut TranscriptItem, label: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        item.details.push(format!("{label}: {value}"));
    }
}

fn number_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|value| {
        value
            .as_i64()
            .map(|number| number.to_string())
            .or_else(|| value.as_u64().map(|number| number.to_string()))
    })
}

fn bool_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
}

fn change_kind(kind: Option<&Value>) -> String {
    let Some(kind) = kind else {
        return "change".to_string();
    };

    if let Some(kind) = kind.as_str() {
        return kind.to_string();
    }

    get_str(kind, &["type"]).unwrap_or("change").to_string()
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<invalid json>".to_string())
}

fn preview(text: &str, max_lines: usize, max_chars: usize) -> String {
    let mut result = String::new();
    let mut truncated = false;

    for (index, line) in text.lines().enumerate() {
        if index >= max_lines {
            truncated = true;
            break;
        }

        if !result.is_empty() {
            result.push('\n');
        }

        let remaining = max_chars.saturating_sub(result.len());
        let line_preview = prefix_by_char_boundary(line, remaining);
        if line_preview.len() < line.len() {
            result.push_str(line_preview);
            truncated = true;
            break;
        }
        result.push_str(line_preview);
    }

    if result.is_empty() && !text.is_empty() {
        let text_preview = prefix_by_char_boundary(text, max_chars);
        result.push_str(text_preview);
        truncated = text_preview.len() < text.len();
    }

    if truncated {
        result.push_str("\n...");
    }

    result
}

fn prefix_by_char_boundary(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }

    let mut end = 0;
    for (index, _) in text.char_indices() {
        if index > max_bytes {
            break;
        }
        end = index;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_result_extracts_success_payload() {
        let result = response_result(json!({
            "id": 1,
            "result": { "thread": { "id": "thr_123" } }
        }))
        .unwrap();

        assert_eq!(get_str(&result, &["thread", "id"]), Some("thr_123"));
    }

    #[test]
    fn response_result_reports_rpc_errors() {
        let err = response_result(json!({
            "id": 1,
            "error": { "code": -32000, "message": "not initialized" }
        }))
        .unwrap_err();

        assert_eq!(err.to_string(), "not initialized");
    }

    #[test]
    fn turn_collector_prefers_completed_agent_message() {
        let mut collector = TurnCollector::new("thr_123", "turn_456");
        collector
            .handle(&json!({
                "method": "item/agentMessage/delta",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "itemId": "item_1",
                    "delta": "Hel"
                }
            }))
            .unwrap();
        collector
            .handle(&json!({
                "method": "item/completed",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "completedAtMs": 1,
                    "item": {
                        "type": "agentMessage",
                        "id": "item_1",
                        "text": "Hello.",
                        "phase": "final_answer"
                    }
                }
            }))
            .unwrap();

        let completed = collector
            .handle(&json!({
                "method": "turn/completed",
                "params": {
                    "threadId": "thr_123",
                    "turn": {
                        "id": "turn_456",
                        "items": [],
                        "status": "completed"
                    }
                }
            }))
            .unwrap()
            .unwrap();

        assert_eq!(
            completed,
            CompletedTurn {
                turn_id: "turn_456".to_string(),
                status: "completed".to_string(),
                transcript: {
                    let mut transcript = TurnTranscript::default();
                    let mut item = TranscriptItem::new(
                        "item_1".to_string(),
                        ItemKind::AgentMessage,
                        "assistant",
                    );
                    item.text = Some("Hello.".to_string());
                    transcript.upsert_item(item);
                    transcript
                },
            }
        );
    }

    #[test]
    fn turn_collector_ignores_other_turns() {
        let mut collector = TurnCollector::new("thr_123", "turn_456");
        let result = collector
            .handle(&json!({
                "method": "turn/completed",
                "params": {
                    "threadId": "thr_123",
                    "turn": {
                        "id": "turn_other",
                        "items": [],
                        "status": "completed"
                    }
                }
            }))
            .unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn turn_collector_records_plan_and_tool_items() {
        let mut collector = TurnCollector::new("thr_123", "turn_456");
        collector
            .handle(&json!({
                "method": "turn/plan/updated",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "plan": [
                        { "step": "Search docs", "status": "completed" },
                        { "step": "Run tests", "status": "inProgress" }
                    ]
                }
            }))
            .unwrap();
        collector
            .handle(&json!({
                "method": "item/completed",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "completedAtMs": 1,
                    "item": {
                        "type": "webSearch",
                        "id": "web_1",
                        "query": "rust cli best practices"
                    }
                }
            }))
            .unwrap();
        collector
            .handle(&json!({
                "method": "item/completed",
                "params": {
                    "threadId": "thr_123",
                    "turnId": "turn_456",
                    "completedAtMs": 2,
                    "item": {
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
                }
            }))
            .unwrap();

        assert_eq!(
            collector.transcript.plan,
            vec![
                PlanStep {
                    status: "completed".to_string(),
                    step: "Search docs".to_string(),
                },
                PlanStep {
                    status: "inProgress".to_string(),
                    step: "Run tests".to_string(),
                },
            ]
        );
        let activity = collector.transcript.activity_items();
        assert_eq!(activity.len(), 2);
        assert_eq!(activity[0].title, "web search: rust cli best practices");
        assert_eq!(activity[1].title, "$ cargo test");
        assert_eq!(activity[1].output, "ok");
    }

    #[test]
    fn transcript_omits_user_messages_and_empty_items() {
        let mut transcript = TurnTranscript::default();
        let user_item = TranscriptItem::from_thread_item(&json!({
            "type": "userMessage",
            "id": "user_1",
            "content": [{ "type": "text", "text": "hi" }]
        }));
        assert_eq!(user_item, None);

        transcript.upsert_item(TranscriptItem::new(
            "reasoning_1".to_string(),
            ItemKind::Reasoning,
            "reasoning",
        ));
        assert!(transcript.activity_items().is_empty());
    }
}
