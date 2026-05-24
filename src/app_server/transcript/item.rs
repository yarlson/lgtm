use serde_json::Value;

use super::{
    CommandExecution, DynamicToolCall, FileChange, McpToolCall, TranscriptItemData, WebSearch,
    data::empty_data,
};
use crate::app_server::{
    json::get_str,
    text::{non_empty, preview},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
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
pub struct TranscriptItem {
    id: String,
    kind: ItemKind,
    pub title: String,
    pub details: Vec<String>,
    status: Option<String>,
    text: Option<String>,
    output: String,
    data: TranscriptItemData,
}

impl TranscriptItem {
    pub(crate) fn placeholder(id: String, kind: ItemKind) -> Self {
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
        Self::new(id, kind, title)
    }

    fn new(id: String, kind: ItemKind, title: impl Into<String>) -> Self {
        Self::new_with_data(id, kind, title, empty_data(kind))
    }

    fn new_with_data(
        id: String,
        kind: ItemKind,
        title: impl Into<String>,
        data: TranscriptItemData,
    ) -> Self {
        Self {
            id,
            kind,
            title: title.into(),
            details: Vec::new(),
            status: None,
            text: None,
            output: String::new(),
            data,
        }
    }

    pub fn from_app_server_item(item: &Value) -> Option<Self> {
        let id = get_str(item, &["id"]).unwrap_or("<missing-id>").to_string();
        match item.get("type").and_then(Value::as_str)? {
            "userMessage" | "hookPrompt" => None,
            "agentMessage" => {
                let text = string_field(item, "text");
                let mut transcript_item = Self::new_with_data(
                    id,
                    ItemKind::AgentMessage,
                    "assistant",
                    TranscriptItemData::AgentMessage { text: text.clone() },
                );
                transcript_item.text = text;
                Some(transcript_item)
            }
            "plan" => {
                let text = string_field(item, "text");
                let mut transcript_item = Self::new_with_data(
                    id,
                    ItemKind::Plan,
                    "plan",
                    TranscriptItemData::Plan { text: text.clone() },
                );
                transcript_item.text = text;
                Some(transcript_item)
            }
            "reasoning" => Some(reasoning_item(id, item)),
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
                let output = compact_json(item);
                let mut transcript_item = Self::new_with_data(
                    id,
                    ItemKind::Other,
                    format!("unknown item: {other}"),
                    TranscriptItemData::Other {
                        details: Vec::new(),
                        output: output.clone(),
                    },
                );
                transcript_item.output = output;
                Some(transcript_item)
            }
        }
    }

    pub(crate) fn from_thread_item(item: &Value) -> Option<Self> {
        Self::from_app_server_item(item)
    }

    pub(crate) fn file_patch_update(id: String, item: &Value) -> Self {
        let changes = file_changes(item);
        let mut transcript_item = Self::new_with_data(
            id,
            ItemKind::FileChange,
            "file changes",
            TranscriptItemData::FileChange {
                changes: changes.clone(),
            },
        );
        transcript_item.details = file_change_details(&changes);
        transcript_item
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn kind(&self) -> ItemKind {
        self.kind
    }

    pub fn item_kind(&self) -> ItemKind {
        self.kind
    }

    pub fn data(&self) -> &TranscriptItemData {
        &self.data
    }

    pub(crate) fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn message_text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub(crate) fn output(&self) -> &str {
        &self.output
    }

    pub fn output_text(&self) -> &str {
        &self.output
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn is_in_progress(&self) -> bool {
        self.status() == Some("inProgress")
    }

    pub fn is_final(&self) -> bool {
        matches!(
            self.status(),
            Some("completed" | "failed" | "declined" | "interrupted")
        )
    }

    pub(crate) fn push_output(&mut self, delta: &str) {
        self.output.push_str(delta);
        match &mut self.data {
            TranscriptItemData::CommandExecution(command) => command.output.push_str(delta),
            TranscriptItemData::Reasoning { content, .. } => content.push_str(delta),
            TranscriptItemData::Other { output, .. } => output.push_str(delta),
            _ => {}
        }
    }

    #[cfg(test)]
    pub(crate) fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.text = Some(text.clone());
        match &mut self.data {
            TranscriptItemData::AgentMessage { text: item_text }
            | TranscriptItemData::Plan { text: item_text } => {
                *item_text = Some(text);
            }
            _ => {}
        }
    }

    pub fn output_preview(&self) -> Option<String> {
        let output = non_empty(&self.output)?;
        Some(preview(output, 12, 2_000))
    }

    pub(crate) fn is_renderable(&self) -> bool {
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

fn reasoning_item(id: String, item: &Value) -> TranscriptItem {
    let summary = string_array_field(item, "summary");
    let content = string_array_field(item, "content");
    let output = if content.is_empty() {
        String::new()
    } else {
        content.join("\n")
    };
    let mut transcript_item = TranscriptItem::new_with_data(
        id,
        ItemKind::Reasoning,
        "reasoning",
        TranscriptItemData::Reasoning {
            summary: summary.clone(),
            content: output.clone(),
        },
    );
    transcript_item.details = summary
        .into_iter()
        .map(|line| format!("summary: {line}"))
        .collect();
    if !content.is_empty() {
        transcript_item.output = content.join("\n");
    }
    transcript_item
}

fn command_item(id: String, item: &Value) -> TranscriptItem {
    let command = string_field(item, "command").unwrap_or_else(|| "<unknown command>".to_string());
    let cwd = string_field(item, "cwd");
    let exit_code = i64_field(item, "exitCode");
    let duration_ms = u64_field(item, "durationMs");
    let output = string_field(item, "aggregatedOutput").unwrap_or_default();
    let mut transcript_item = TranscriptItem::new_with_data(
        id,
        ItemKind::CommandExecution,
        format!("$ {command}"),
        TranscriptItemData::CommandExecution(CommandExecution {
            command: command.clone(),
            cwd: cwd.clone(),
            exit_code,
            duration_ms,
            output: output.clone(),
        }),
    );
    record_status(&mut transcript_item, item);
    push_detail(&mut transcript_item, "cwd", cwd);
    push_detail(
        &mut transcript_item,
        "exit",
        exit_code.map(|value| value.to_string()),
    );
    push_detail(
        &mut transcript_item,
        "duration_ms",
        duration_ms.map(|value| value.to_string()),
    );
    transcript_item.output = output;
    transcript_item
}

fn file_change_item(id: String, item: &Value) -> TranscriptItem {
    let changes = file_changes(item);
    let mut transcript_item = TranscriptItem::new_with_data(
        id,
        ItemKind::FileChange,
        "file changes",
        TranscriptItemData::FileChange {
            changes: changes.clone(),
        },
    );
    record_status(&mut transcript_item, item);
    transcript_item
        .details
        .extend(file_change_details(&changes));
    transcript_item
}

fn file_changes(value: &Value) -> Vec<FileChange> {
    value
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|change| {
            let path = string_field(change, "path")?;
            let kind = change_kind(change.get("kind"));
            Some(FileChange { kind, path })
        })
        .collect()
}

fn file_change_details(changes: &[FileChange]) -> Vec<String> {
    changes
        .iter()
        .map(|change| format!("{}: {}", change.kind, change.path))
        .collect()
}

fn web_search_item(id: String, item: &Value) -> TranscriptItem {
    let query = string_field(item, "query").unwrap_or_else(|| "<empty query>".to_string());
    let action = item
        .get("action")
        .filter(|action| !action.is_null())
        .map(|action| preview(&compact_json(action), 1, 500));
    let mut transcript_item = TranscriptItem::new_with_data(
        id,
        ItemKind::WebSearch,
        format!("web search: {query}"),
        TranscriptItemData::WebSearch(WebSearch {
            query: query.clone(),
            action: action.clone(),
        }),
    );
    record_status(&mut transcript_item, item);
    if let Some(action) = action {
        transcript_item.details.push(format!("action: {action}"));
    }
    transcript_item
}

fn mcp_tool_item(id: String, item: &Value) -> TranscriptItem {
    let server = string_field(item, "server").unwrap_or_else(|| "<unknown server>".to_string());
    let tool = string_field(item, "tool").unwrap_or_else(|| "<unknown tool>".to_string());
    let duration_ms = u64_field(item, "durationMs");
    let arguments = item
        .get("arguments")
        .map(|arguments| preview(&compact_json(arguments), 1, 500));
    let error = item
        .get("error")
        .filter(|error| !error.is_null())
        .map(|error| preview(&compact_json(error), 1, 500));
    let result = item
        .get("result")
        .filter(|result| !result.is_null())
        .map(|result| preview(&compact_json(result), 20, 2_000))
        .unwrap_or_default();
    let mut transcript_item = TranscriptItem::new_with_data(
        id,
        ItemKind::McpToolCall,
        format!("mcp {server}/{tool}"),
        TranscriptItemData::McpToolCall(McpToolCall {
            server: server.clone(),
            tool: tool.clone(),
            duration_ms,
            arguments: arguments.clone(),
            error: error.clone(),
            result: result.clone(),
        }),
    );
    record_status(&mut transcript_item, item);
    push_detail(
        &mut transcript_item,
        "duration_ms",
        duration_ms.map(|value| value.to_string()),
    );
    if let Some(arguments) = arguments {
        transcript_item
            .details
            .push(format!("arguments: {arguments}"));
    }
    if let Some(error) = error {
        transcript_item.details.push(format!("error: {error}"));
    }
    transcript_item.output = result;
    transcript_item
}

fn dynamic_tool_item(id: String, item: &Value) -> TranscriptItem {
    let tool = string_field(item, "tool").unwrap_or_else(|| "<unknown tool>".to_string());
    let namespace = string_field(item, "namespace");
    let title = match &namespace {
        Some(namespace) => format!("tool {namespace}/{tool}"),
        None => format!("tool {tool}"),
    };
    let success = item.get("success").and_then(Value::as_bool);
    let duration_ms = u64_field(item, "durationMs");
    let arguments = item
        .get("arguments")
        .map(|arguments| preview(&compact_json(arguments), 1, 500));
    let content = item
        .get("contentItems")
        .filter(|content| !content.is_null())
        .map(|content| preview(&compact_json(content), 20, 2_000))
        .unwrap_or_default();
    let mut transcript_item = TranscriptItem::new_with_data(
        id,
        ItemKind::DynamicToolCall,
        title,
        TranscriptItemData::DynamicToolCall(DynamicToolCall {
            namespace,
            tool: tool.clone(),
            success,
            duration_ms,
            arguments: arguments.clone(),
            content: content.clone(),
        }),
    );
    record_status(&mut transcript_item, item);
    push_detail(
        &mut transcript_item,
        "success",
        success.map(|value| value.to_string()),
    );
    push_detail(
        &mut transcript_item,
        "duration_ms",
        duration_ms.map(|value| value.to_string()),
    );
    if let Some(arguments) = arguments {
        transcript_item
            .details
            .push(format!("arguments: {arguments}"));
    }
    transcript_item.output = content;
    transcript_item
}

fn collab_tool_item(id: String, item: &Value) -> TranscriptItem {
    let tool = string_field(item, "tool").unwrap_or_else(|| "<unknown tool>".to_string());
    let prompt = string_field(item, "prompt").map(|prompt| preview(&prompt, 2, 500));
    let mut transcript_item = TranscriptItem::new_with_data(
        id,
        ItemKind::DynamicToolCall,
        format!("agent tool {tool}"),
        TranscriptItemData::DynamicToolCall(DynamicToolCall {
            namespace: Some("agent".to_string()),
            tool: tool.clone(),
            success: None,
            duration_ms: None,
            arguments: prompt.clone(),
            content: String::new(),
        }),
    );
    record_status(&mut transcript_item, item);
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
    if let Some(prompt) = prompt {
        transcript_item.details.push(format!("prompt: {prompt}"));
    }
    transcript_item
}

fn simple_path_item(id: String, kind: ItemKind, title: &str, item: &Value) -> TranscriptItem {
    let mut transcript_item = TranscriptItem::new(id, kind, title);
    push_detail(&mut transcript_item, "path", string_field(item, "path"));
    transcript_item.data = TranscriptItemData::Other {
        details: transcript_item.details.clone(),
        output: String::new(),
    };
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
    transcript_item.data = TranscriptItemData::Other {
        details: Vec::new(),
        output: transcript_item.output.clone(),
    };
    transcript_item
}

fn push_detail(item: &mut TranscriptItem, label: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        item.details.push(format!("{label}: {value}"));
    }
}

fn record_status(item: &mut TranscriptItem, value: &Value) {
    item.status = string_field(value, "status");
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

fn i64_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
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
