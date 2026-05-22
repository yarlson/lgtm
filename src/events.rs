use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct CodexEvent {
    pub event_type: String,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodexItem {
    pub id: String,
    pub item_type: String,
    pub raw: Value,
}

impl CodexEvent {
    pub fn parse(line: &str) -> Result<Self, serde_json::Error> {
        let raw: Value = serde_json::from_str(line)?;
        let event_type = raw
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        Ok(Self { event_type, raw })
    }

    pub fn item(&self) -> Option<CodexItem> {
        let raw = self.raw.get("item")?.clone();
        let item_type = raw.get("type")?.as_str()?.to_string();
        let id = raw
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Some(CodexItem { id, item_type, raw })
    }

    pub fn string_at(&self, key: &str) -> Option<&str> {
        self.raw.get(key).and_then(Value::as_str)
    }

    pub fn usage(&self) -> Usage {
        let usage = self.raw.get("usage");
        Usage {
            input_tokens: number_at(usage, "input_tokens"),
            cached_input_tokens: number_at(usage, "cached_input_tokens"),
            output_tokens: number_at(usage, "output_tokens"),
            reasoning_output_tokens: number_at(usage, "reasoning_output_tokens"),
        }
    }

    pub fn error_message(&self) -> Option<String> {
        self.raw
            .get("error")
            .and_then(|error| error.get("message"))
            .or_else(|| self.raw.get("message"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
}

impl CodexItem {
    pub fn string_at(&self, key: &str) -> Option<&str> {
        self.raw.get(key).and_then(Value::as_str)
    }

    pub fn status(&self) -> Option<&str> {
        self.string_at("status")
    }

    pub fn text(&self) -> Option<&str> {
        self.string_at("text")
    }

    pub fn command_output(&self) -> Option<&str> {
        self.string_at("aggregated_output")
    }

    pub fn exit_code(&self) -> Option<i64> {
        self.raw.get("exit_code").and_then(Value::as_i64)
    }

    pub fn changes(&self) -> Vec<FileChange> {
        self.raw
            .get("changes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|change| {
                let path = change.get("path")?.as_str()?.to_string();
                let kind = change
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("update")
                    .to_string();
                Some(FileChange { path, kind })
            })
            .collect()
    }

    pub fn todos(&self) -> Vec<TodoItem> {
        self.raw
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let text = item.get("text")?.as_str()?.to_string();
                let completed = item
                    .get("completed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                Some(TodoItem { text, completed })
            })
            .collect()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.raw
            .get("error")
            .and_then(|error| error.get("message"))
            .or_else(|| self.raw.get("message"))
            .and_then(Value::as_str)
    }

    pub fn receiver_count(&self) -> usize {
        self.raw
            .get("receiver_thread_ids")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0)
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

fn number_at(parent: Option<&Value>, key: &str) -> i64 {
    parent
        .and_then(|parent| parent.get(key))
        .and_then(Value::as_i64)
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

        let item = event.item().unwrap();
        assert_eq!(event.event_type, "item.completed");
        assert_eq!(item.item_type, "command_execution");
        assert_eq!(item.string_at("command"), Some("cargo check"));
        assert_eq!(item.command_output(), Some("ok\n"));
        assert_eq!(item.exit_code(), Some(0));
    }
}
