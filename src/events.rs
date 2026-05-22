#[derive(Debug, Clone, PartialEq)]
pub struct CodexEvent {
    pub event_type: String,
    pub kind: EventKind,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexItem {
    pub id: String,
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
    Malformed { reason: String },
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
    Malformed {
        item_type: String,
        reason: String,
    },
    Unknown {
        item_type: String,
    },
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
enum ItemKind {
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
        EventKind::ThreadStarted => match string_at(raw, "thread_id") {
            Some(thread_id) => EventPayload::ThreadStarted { thread_id },
            None => malformed("thread.started missing thread_id"),
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
        EventKind::ItemStarted | EventKind::ItemUpdated | EventKind::ItemCompleted => {
            match raw.get("item") {
                Some(item) => match parse_item(item) {
                    Ok(item) => EventPayload::Item { item },
                    Err(reason) => malformed(reason),
                },
                None => malformed("item event missing item"),
            }
        }
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

fn parse_item(raw: &serde_json::Value) -> Result<CodexItem, String> {
    let item_type = require_string(raw, "type", "item")?;
    let kind = ItemKind::from_str(&item_type);
    let status = ItemStatus::from_value(raw.get("status"));
    let id = string_at(raw, "id").unwrap_or_default();
    let payload = item_payload(kind, &item_type, raw);
    Ok(CodexItem {
        id,
        status,
        payload,
    })
}

fn item_payload(kind: ItemKind, item_type: &str, raw: &serde_json::Value) -> ItemPayload {
    match kind {
        ItemKind::AgentMessage => required_item_string(raw, item_type, "text")
            .map(|text| ItemPayload::AgentMessage { text })
            .unwrap_or_else(malformed_item),
        ItemKind::Reasoning => required_item_string(raw, item_type, "text")
            .map(|text| ItemPayload::Reasoning { text })
            .unwrap_or_else(malformed_item),
        ItemKind::CommandExecution => required_item_string(raw, item_type, "command")
            .map(|command| ItemPayload::CommandExecution {
                command,
                output: string_at(raw, "aggregated_output"),
                exit_code: raw.get("exit_code").and_then(serde_json::Value::as_i64),
            })
            .unwrap_or_else(malformed_item),
        ItemKind::FileChange => file_changes(raw, item_type)
            .map(|changes| ItemPayload::FileChange { changes })
            .unwrap_or_else(malformed_item),
        ItemKind::McpToolCall => match (
            required_item_string(raw, item_type, "server"),
            required_item_string(raw, item_type, "tool"),
        ) {
            (Ok(server), Ok(tool)) => ItemPayload::McpToolCall {
                server,
                tool,
                error_message: error_message(raw),
            },
            (Err(error), _) | (_, Err(error)) => malformed_item(error),
        },
        ItemKind::CollabToolCall => required_item_string(raw, item_type, "tool")
            .map(|tool| ItemPayload::CollabToolCall {
                tool,
                receiver_count: raw
                    .get("receiver_thread_ids")
                    .and_then(serde_json::Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
            })
            .unwrap_or_else(malformed_item),
        ItemKind::WebSearch => required_item_string(raw, item_type, "query")
            .map(|query| ItemPayload::WebSearch { query })
            .unwrap_or_else(malformed_item),
        ItemKind::TodoList => todo_items(raw, item_type)
            .map(|items| ItemPayload::TodoList { items })
            .unwrap_or_else(malformed_item),
        ItemKind::Error => ItemPayload::Error {
            message: error_message(raw),
        },
        ItemKind::Unknown => ItemPayload::Unknown {
            item_type: item_type.to_string(),
        },
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

fn file_changes(raw: &serde_json::Value, item_type: &str) -> Result<Vec<FileChange>, ItemError> {
    let changes = raw
        .get("changes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ItemError::missing(item_type, "changes"))?;

    Ok(changes
        .iter()
        .filter_map(|change| {
            let path = change.get("path")?.as_str()?.to_string();
            let kind = change
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("update")
                .to_string();
            Some(FileChange { path, kind })
        })
        .collect())
}

fn todo_items(raw: &serde_json::Value, item_type: &str) -> Result<Vec<TodoItem>, ItemError> {
    let items = raw
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ItemError::missing(item_type, "items"))?;

    Ok(items
        .iter()
        .filter_map(|item| {
            let text = item.get("text")?.as_str()?.to_string();
            let completed = item
                .get("completed")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            Some(TodoItem { text, completed })
        })
        .collect())
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

fn require_string(raw: &serde_json::Value, key: &str, owner: &str) -> Result<String, String> {
    string_at(raw, key).ok_or_else(|| format!("{owner} missing {key}"))
}

fn required_item_string(
    raw: &serde_json::Value,
    item_type: &str,
    key: &str,
) -> Result<String, ItemError> {
    string_at(raw, key).ok_or_else(|| ItemError::missing(item_type, key))
}

fn malformed(reason: impl Into<String>) -> EventPayload {
    EventPayload::Malformed {
        reason: reason.into(),
    }
}

fn malformed_item(error: ItemError) -> ItemPayload {
    ItemPayload::Malformed {
        item_type: error.item_type,
        reason: error.reason,
    }
}

struct ItemError {
    item_type: String,
    reason: String,
}

impl ItemError {
    fn missing(item_type: &str, field: &str) -> Self {
        Self {
            item_type: item_type.to_string(),
            reason: format!("{item_type} missing {field}"),
        }
    }
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

    #[test]
    fn preserves_unknown_item_type_inside_unknown_payload() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"new_tool_call","status":"completed"}}"#,
        )
        .unwrap();

        let EventPayload::Item { item } = event.payload else {
            panic!("expected item payload");
        };
        assert_eq!(
            item.payload,
            ItemPayload::Unknown {
                item_type: "new_tool_call".to_string()
            }
        );
    }

    #[test]
    fn marks_malformed_known_item_payloads() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","status":"completed"}}"#,
        )
        .unwrap();

        let EventPayload::Item { item } = event.payload else {
            panic!("expected item payload");
        };
        assert_eq!(
            item.payload,
            ItemPayload::Malformed {
                item_type: "command_execution".to_string(),
                reason: "command_execution missing command".to_string(),
            }
        );
    }

    #[test]
    fn marks_malformed_known_events() {
        let event = CodexEvent::parse(r#"{"type":"thread.started"}"#).unwrap();

        assert_eq!(
            event.payload,
            EventPayload::Malformed {
                reason: "thread.started missing thread_id".to_string(),
            }
        );
    }
}
