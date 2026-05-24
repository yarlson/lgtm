use serde_json::Value;

use crate::{
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
    text: Option<String>,
    output: String,
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
        Self {
            id,
            kind,
            title: title.into(),
            details: Vec::new(),
            text: None,
            output: String::new(),
        }
    }

    pub(crate) fn from_thread_item(item: &Value) -> Option<Self> {
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
                let mut transcript_item =
                    Self::new(id, ItemKind::Other, format!("unknown item: {other}"));
                transcript_item.output = compact_json(item);
                Some(transcript_item)
            }
        }
    }

    pub(crate) fn file_patch_update(id: String, item: &Value) -> Self {
        let mut transcript_item = Self::new(id, ItemKind::FileChange, "file changes");
        transcript_item.details = file_change_details(item);
        transcript_item
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn kind(&self) -> ItemKind {
        self.kind
    }

    pub(crate) fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub(crate) fn output(&self) -> &str {
        &self.output
    }

    pub(crate) fn push_output(&mut self, delta: &str) {
        self.output.push_str(delta);
    }

    #[cfg(test)]
    pub(crate) fn set_text(&mut self, text: impl Into<String>) {
        self.text = Some(text.into());
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
    let mut transcript_item = TranscriptItem::new(id, ItemKind::Reasoning, "reasoning");
    transcript_item.details = string_array_field(item, "summary")
        .into_iter()
        .map(|line| format!("summary: {line}"))
        .collect();
    let content = string_array_field(item, "content");
    if !content.is_empty() {
        transcript_item.output = content.join("\n");
    }
    transcript_item
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
