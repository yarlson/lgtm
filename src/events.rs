#[derive(Debug, Clone, PartialEq)]
pub struct CodexEvent {
    pub event_type: String,
    pub kind: EventKind,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexItem {
    pub id: String,
    pub item_type: String,
    pub kind: ItemKind,
    pub status: ItemStatus,
    pub payload: ItemPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventPayload {
    ThreadStarted { thread_id: String },
    TurnStarted,
    TurnCompleted { usage: Usage },
    TurnFailed { message: Option<String> },
    Error { message: Option<String> },
    Item { item: CodexItem },
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemPayload {
    AgentMessage {
        text: String,
    },
    Reasoning {
        text: String,
    },
    CommandExecution {
        command: String,
        output: Option<String>,
        exit_code: Option<i64>,
    },
    FileChange {
        changes: Vec<FileChange>,
    },
    McpToolCall {
        server: String,
        tool: String,
        error_message: Option<String>,
    },
    CollabToolCall {
        tool: String,
        receiver_count: usize,
    },
    WebSearch {
        query: String,
    },
    TodoList {
        items: Vec<TodoItem>,
    },
    Error {
        message: Option<String>,
    },
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    ThreadStarted,
    TurnStarted,
    TurnCompleted,
    TurnFailed,
    Error,
    ItemStarted,
    ItemUpdated,
    ItemCompleted,
    Unknown,
}

impl EventKind {
    fn from_str(value: &str) -> Self {
        match value {
            "thread.started" => Self::ThreadStarted,
            "turn.started" => Self::TurnStarted,
            "turn.completed" => Self::TurnCompleted,
            "turn.failed" => Self::TurnFailed,
            "error" => Self::Error,
            "item.started" => Self::ItemStarted,
            "item.updated" => Self::ItemUpdated,
            "item.completed" => Self::ItemCompleted,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    AgentMessage,
    Reasoning,
    CommandExecution,
    FileChange,
    McpToolCall,
    CollabToolCall,
    WebSearch,
    TodoList,
    Error,
    Unknown,
}

impl ItemKind {
    fn from_str(value: &str) -> Self {
        match value {
            "agent_message" => Self::AgentMessage,
            "reasoning" => Self::Reasoning,
            "command_execution" => Self::CommandExecution,
            "file_change" => Self::FileChange,
            "mcp_tool_call" => Self::McpToolCall,
            "collab_tool_call" => Self::CollabToolCall,
            "web_search" => Self::WebSearch,
            "todo_list" => Self::TodoList,
            "error" => Self::Error,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    Completed,
    Failed,
    Declined,
    InProgress,
    Missing,
    Unknown,
}

impl ItemStatus {
    fn from_value(value: Option<&serde_json::Value>) -> Self {
        match value.and_then(serde_json::Value::as_str) {
            Some("completed") => Self::Completed,
            Some("failed") => Self::Failed,
            Some("declined") => Self::Declined,
            Some("in_progress") => Self::InProgress,
            Some(_) => Self::Unknown,
            None => Self::Missing,
        }
    }
}

impl CodexEvent {
    pub fn parse(line: &str) -> Result<Self, serde_json::Error> {
        let raw: serde_json::Value = serde_json::from_str(line)?;
        let event_type = raw
            .get("type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let kind = EventKind::from_str(&event_type);
        let payload = event_payload(kind, &raw);
        Ok(Self {
            event_type,
            kind,
            payload,
        })
    }
}

fn event_payload(kind: EventKind, raw: &serde_json::Value) -> EventPayload {
    match kind {
        EventKind::ThreadStarted => EventPayload::ThreadStarted {
            thread_id: string_at(raw, "thread_id").unwrap_or_default(),
        },
        EventKind::TurnStarted => EventPayload::TurnStarted,
        EventKind::TurnCompleted => EventPayload::TurnCompleted {
            usage: usage(raw.get("usage")),
        },
        EventKind::TurnFailed => EventPayload::TurnFailed {
            message: error_message(raw),
        },
        EventKind::Error => EventPayload::Error {
            message: error_message(raw),
        },
        EventKind::ItemStarted | EventKind::ItemUpdated | EventKind::ItemCompleted => raw
            .get("item")
            .and_then(parse_item)
            .map(|item| EventPayload::Item { item })
            .unwrap_or(EventPayload::Unknown),
        EventKind::Unknown => EventPayload::Unknown,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
}

fn parse_item(raw: &serde_json::Value) -> Option<CodexItem> {
    let item_type = string_at(raw, "type")?;
    let kind = ItemKind::from_str(&item_type);
    let status = ItemStatus::from_value(raw.get("status"));
    let id = string_at(raw, "id").unwrap_or_default();
    let payload = item_payload(kind, raw);
    Some(CodexItem {
        id,
        item_type,
        kind,
        status,
        payload,
    })
}

fn item_payload(kind: ItemKind, raw: &serde_json::Value) -> ItemPayload {
    match kind {
        ItemKind::AgentMessage => ItemPayload::AgentMessage {
            text: string_at(raw, "text").unwrap_or_default(),
        },
        ItemKind::Reasoning => ItemPayload::Reasoning {
            text: string_at(raw, "text").unwrap_or_default(),
        },
        ItemKind::CommandExecution => ItemPayload::CommandExecution {
            command: string_at(raw, "command").unwrap_or_default(),
            output: string_at(raw, "aggregated_output"),
            exit_code: raw.get("exit_code").and_then(serde_json::Value::as_i64),
        },
        ItemKind::FileChange => ItemPayload::FileChange {
            changes: file_changes(raw),
        },
        ItemKind::McpToolCall => ItemPayload::McpToolCall {
            server: string_at(raw, "server").unwrap_or_default(),
            tool: string_at(raw, "tool").unwrap_or_default(),
            error_message: error_message(raw),
        },
        ItemKind::CollabToolCall => ItemPayload::CollabToolCall {
            tool: string_at(raw, "tool").unwrap_or_else(|| "unknown".to_string()),
            receiver_count: raw
                .get("receiver_thread_ids")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len)
                .unwrap_or(0),
        },
        ItemKind::WebSearch => ItemPayload::WebSearch {
            query: string_at(raw, "query").unwrap_or_default(),
        },
        ItemKind::TodoList => ItemPayload::TodoList {
            items: todo_items(raw),
        },
        ItemKind::Error => ItemPayload::Error {
            message: error_message(raw),
        },
        ItemKind::Unknown => ItemPayload::Unknown,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    pub text: String,
    pub completed: bool,
}

fn usage(parent: Option<&serde_json::Value>) -> Usage {
    Usage {
        input_tokens: number_at(parent, "input_tokens"),
        cached_input_tokens: number_at(parent, "cached_input_tokens"),
        output_tokens: number_at(parent, "output_tokens"),
        reasoning_output_tokens: number_at(parent, "reasoning_output_tokens"),
    }
}

fn file_changes(raw: &serde_json::Value) -> Vec<FileChange> {
    raw.get("changes")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|change| {
            let path = change.get("path")?.as_str()?.to_string();
            let kind = change
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("update")
                .to_string();
            Some(FileChange { path, kind })
        })
        .collect()
}

fn todo_items(raw: &serde_json::Value) -> Vec<TodoItem> {
    raw.get("items")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let text = item.get("text")?.as_str()?.to_string();
            let completed = item
                .get("completed")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some(TodoItem { text, completed })
        })
        .collect()
}

fn error_message(raw: &serde_json::Value) -> Option<String> {
    raw.get("error")
        .and_then(|error| error.get("message"))
        .or_else(|| raw.get("message"))
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

fn string_at(raw: &serde_json::Value, key: &str) -> Option<String> {
    raw.get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

fn number_at(parent: Option<&serde_json::Value>, key: &str) -> i64 {
    parent
        .and_then(|parent| parent.get(key))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flattened_item_type_from_codex_jsonl() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"ok\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();

        assert_eq!(event.event_type, "item.completed");
        assert_eq!(event.kind, EventKind::ItemCompleted);
        let EventPayload::Item { item } = event.payload else {
            panic!("expected item payload");
        };
        assert_eq!(item.item_type, "command_execution");
        assert_eq!(item.kind, ItemKind::CommandExecution);
        assert_eq!(item.status, ItemStatus::Completed);
        assert_eq!(
            item.payload,
            ItemPayload::CommandExecution {
                command: "cargo check".to_string(),
                output: Some("ok\n".to_string()),
                exit_code: Some(0),
            }
        );
    }
}
