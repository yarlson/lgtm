use super::item::ItemKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptItemData {
    AgentMessage {
        text: Option<String>,
    },
    Plan {
        text: Option<String>,
    },
    Reasoning {
        summary: Vec<String>,
        content: String,
    },
    CommandExecution(CommandExecution),
    FileChange {
        changes: Vec<FileChange>,
    },
    WebSearch(WebSearch),
    McpToolCall(McpToolCall),
    DynamicToolCall(DynamicToolCall),
    Other {
        details: Vec<String>,
        output: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandExecution {
    pub command: String,
    pub cwd: Option<String>,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<u64>,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub kind: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSearch {
    pub query: String,
    pub action: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpToolCall {
    pub server: String,
    pub tool: String,
    pub duration_ms: Option<u64>,
    pub arguments: Option<String>,
    pub error: Option<String>,
    pub result: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicToolCall {
    pub namespace: Option<String>,
    pub tool: String,
    pub success: Option<bool>,
    pub duration_ms: Option<u64>,
    pub arguments: Option<String>,
    pub content: String,
}

pub(crate) fn empty_data(kind: ItemKind) -> TranscriptItemData {
    match kind {
        ItemKind::AgentMessage => TranscriptItemData::AgentMessage { text: None },
        ItemKind::Plan => TranscriptItemData::Plan { text: None },
        ItemKind::Reasoning => TranscriptItemData::Reasoning {
            summary: Vec::new(),
            content: String::new(),
        },
        ItemKind::CommandExecution => TranscriptItemData::CommandExecution(CommandExecution {
            command: "<unknown command>".to_string(),
            cwd: None,
            exit_code: None,
            duration_ms: None,
            output: String::new(),
        }),
        ItemKind::FileChange => TranscriptItemData::FileChange {
            changes: Vec::new(),
        },
        ItemKind::WebSearch => TranscriptItemData::WebSearch(WebSearch {
            query: "<empty query>".to_string(),
            action: None,
        }),
        ItemKind::McpToolCall => TranscriptItemData::McpToolCall(McpToolCall {
            server: "<unknown server>".to_string(),
            tool: "<unknown tool>".to_string(),
            duration_ms: None,
            arguments: None,
            error: None,
            result: String::new(),
        }),
        ItemKind::DynamicToolCall => TranscriptItemData::DynamicToolCall(DynamicToolCall {
            namespace: None,
            tool: "<unknown tool>".to_string(),
            success: None,
            duration_ms: None,
            arguments: None,
            content: String::new(),
        }),
        ItemKind::Other => TranscriptItemData::Other {
            details: Vec::new(),
            output: String::new(),
        },
    }
}
