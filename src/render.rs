mod format;

use termimad::MadSkin;

use crate::events::CodexEvent;
use crate::events::CodexItem;
use crate::events::EventPayload;
use crate::events::FileChange;
use crate::events::ItemPayload;
use crate::events::ItemStatus;
use crate::events::TodoItem;
use crate::events::Usage;
use crate::render::format::blank;
use crate::render::format::block;
use crate::render::format::child;
use crate::render::format::continuation;
use crate::render::format::file_line;
use crate::render::format::header;
use crate::render::format::line_count;
use crate::render::format::output_lines;
use crate::render::format::raw_body_line;
use crate::render::format::separator;
use crate::render::format::text;
use crate::terminal::Color;
use crate::terminal::Text;
use crate::terminal::text_to_string;

#[derive(Debug, Clone)]
pub struct Renderer {
    color: bool,
}

impl Renderer {
    pub fn new() -> Self {
        let color = supports_color::on_cached(supports_color::Stream::Stdout).is_some()
            && std::env::var_os("NO_COLOR").is_none();
        Self { color }
    }

    #[cfg(test)]
    pub fn without_color() -> Self {
        Self { color: false }
    }

    pub fn phase_header(&self, phase: u32, title: &str, action: &str, log_path: &str) {
        self.emit(text(vec![
            header(
                "Ran",
                Color::LightBlue,
                format!("phase={phase:02} pass={action} title=\"{title}\""),
            ),
            child(format!("raw_jsonl {log_path}")),
            blank(),
        ]));
    }

    pub fn system(&self, message: impl Into<String>) {
        self.emit(text(vec![
            header("Snap-rs", Color::DarkGray, message),
            blank(),
        ]));
    }

    pub fn sleep(&self, seconds: u64, next_phase: u32) {
        self.emit(text(vec![
            header(
                "Waiting",
                Color::DarkGray,
                format!("{seconds}s before Phase {next_phase}"),
            ),
            blank(),
        ]));
    }

    pub fn raw_parse_error(&self, raw_line: &str, error: &serde_json::Error) {
        self.emit(text(vec![
            header(
                "Warning",
                Color::Yellow,
                format!("json parse_error=\"{error}\""),
            ),
            child(raw_line.trim_end()),
            blank(),
        ]));
    }

    pub fn event(&self, event: &CodexEvent) {
        self.emit(render_event(event, self.color));
    }

    #[cfg(test)]
    pub fn render_to_string(&self, event: &CodexEvent) -> String {
        text_to_string(render_event(event, self.color), self.color)
    }

    fn emit(&self, rendered: Text) {
        print!("{}", text_to_string(rendered, self.color));
    }
}

fn render_event(event: &CodexEvent, color: bool) -> Text {
    match &event.payload {
        EventPayload::ThreadStarted { thread_id } => {
            block("Ran", Color::Cyan, format!("thread {thread_id}"))
        }
        EventPayload::TurnStarted => block("Ran", Color::Cyan, "turn begin"),
        EventPayload::TurnCompleted { usage } => render_turn_completed(*usage),
        EventPayload::TurnFailed { message } => block(
            "Failed",
            Color::Red,
            format!(
                "turn error=\"{}\"",
                message.as_deref().unwrap_or("unknown error")
            ),
        ),
        EventPayload::Error { message } => block(
            "Failed",
            Color::Red,
            format!("codex {}", message.as_deref().unwrap_or("unknown error")),
        ),
        EventPayload::Item { item } => render_item(item, color),
        EventPayload::Malformed { reason } => {
            block("Warning", Color::Yellow, format!("json {reason}"))
        }
        EventPayload::Unknown => block("Event", Color::DarkGray, event.event_type.clone()),
    }
}

fn render_turn_completed(usage: Usage) -> Text {
    block(
        "Verification",
        Color::Green,
        format!(
            "tokens input={} cached={} output={} reasoning={}",
            usage.input_tokens,
            usage.cached_input_tokens,
            usage.output_tokens,
            usage.reasoning_output_tokens
        ),
    )
}

fn render_item(item: &CodexItem, color: bool) -> Text {
    match &item.payload {
        ItemPayload::AgentMessage { text } => render_agent_message(text, color),
        ItemPayload::Reasoning { text } => render_reasoning(text),
        ItemPayload::CommandExecution {
            command,
            output,
            exit_code,
        } => render_command(item.status, command, output.as_deref(), *exit_code),
        ItemPayload::FileChange { changes } => render_file_change(item.status, changes),
        ItemPayload::McpToolCall {
            server,
            tool,
            error_message,
        } => render_mcp(item.status, server, tool, error_message.as_deref()),
        ItemPayload::CollabToolCall {
            tool,
            receiver_count,
        } => render_collab(item.status, tool, *receiver_count),
        ItemPayload::WebSearch { query } => render_web_search(query),
        ItemPayload::TodoList { items } => render_todos(items),
        ItemPayload::Error { message } => block(
            "Failed",
            status_color(item.status),
            format!("codex {}", message.as_deref().unwrap_or("unknown error")),
        ),
        ItemPayload::Malformed {
            item_type,
            reason: _,
        } if is_progress_only_search(item.status, item_type, "") => text(Vec::new()),
        ItemPayload::Malformed { item_type, reason } => block(
            "Warning",
            Color::Yellow,
            format!("{item_type} malformed: {reason}"),
        ),
        ItemPayload::Unknown { item_type } => block("Event", Color::DarkGray, item_type),
    }
}

fn render_agent_message(message: &str, color: bool) -> Text {
    let mut lines = vec![
        separator(),
        blank(),
        header("Codex", Color::LightMagenta, ""),
    ];
    lines.extend(markdown_lines(message, color));
    lines.push(blank());
    text(lines)
}

fn render_command(
    status: ItemStatus,
    command: &str,
    output: Option<&str>,
    exit_code: Option<i64>,
) -> Text {
    if is_progress_only_command(status, output, exit_code) {
        return text(Vec::new());
    }

    let mut lines = vec![header("Ran", status_color(status), command)];
    lines.extend(output_lines(output));
    if let Some(exit_code) = exit_code.filter(|code| *code != 0) {
        lines.push(continuation(format!("exit={exit_code}")));
    }
    lines.push(blank());
    text(lines)
}

fn is_progress_only_command(
    status: ItemStatus,
    output: Option<&str>,
    exit_code: Option<i64>,
) -> bool {
    is_progress_status(status)
        && output.is_none_or(|output| output.trim().is_empty())
        && exit_code.is_none()
}

fn render_web_search(query: &str) -> Text {
    if query.trim().is_empty() {
        text(Vec::new())
    } else {
        block("Searched", Color::LightBlue, query)
    }
}

fn is_progress_only_search(status: ItemStatus, item_type: &str, query: &str) -> bool {
    item_type == "web_search" && is_progress_status(status) && query.trim().is_empty()
}

fn is_progress_status(status: ItemStatus) -> bool {
    matches!(
        status,
        ItemStatus::InProgress | ItemStatus::Missing | ItemStatus::Unknown
    )
}

fn render_file_change(status: ItemStatus, changes: &[FileChange]) -> Text {
    let title = if changes.len() == 1 {
        changes[0].path.as_str()
    } else {
        "files"
    };
    let mut lines = vec![header("Edited", status_color(status), title)];

    for change in changes {
        let marker = match change.kind.as_str() {
            "add" => "+",
            "delete" => "-",
            _ => "~",
        };
        let color = match marker {
            "+" => Color::Green,
            "-" => Color::Red,
            _ => Color::Yellow,
        };
        lines.push(file_line(marker, color, change.path.as_str()));
    }

    lines.push(blank());
    text(lines)
}

fn render_mcp(status: ItemStatus, server: &str, tool: &str, error_message: Option<&str>) -> Text {
    let mut lines = vec![header(
        "Ran",
        status_color(status),
        format!("{server}/{tool}"),
    )];
    if let Some(error) = error_message {
        lines.push(child(error));
    }
    lines.push(blank());
    text(lines)
}

fn render_collab(status: ItemStatus, tool: &str, receiver_count: usize) -> Text {
    block(
        "Ran",
        status_color(status),
        format!("{tool} receiver_threads={receiver_count}"),
    )
}

fn render_todos(todos: &[TodoItem]) -> Text {
    let mut lines = vec![header("Updated Plan", Color::LightBlue, "")];
    for todo in todos {
        lines.push(format::checklist_line(todo.completed, todo.text.as_str()));
    }
    lines.push(blank());
    text(lines)
}

fn render_reasoning(reasoning: &str) -> Text {
    block(
        "Reasoning",
        Color::DarkGray,
        format!("… {} lines hidden", line_count(reasoning)),
    )
}

fn markdown_lines(markdown: &str, _color: bool) -> Vec<crate::terminal::Line> {
    if markdown.trim().is_empty() {
        return Vec::new();
    }

    let skin = if _color {
        MadSkin::default_dark()
    } else {
        MadSkin::no_style()
    };

    skin.term_text(markdown)
        .to_string()
        .lines()
        .map(|line| {
            if line.is_empty() {
                blank()
            } else {
                raw_body_line(line)
            }
        })
        .collect()
}

fn status_color(status: ItemStatus) -> Color {
    match status {
        ItemStatus::Completed => Color::Green,
        ItemStatus::Failed => Color::Red,
        ItemStatus::Declined => Color::Yellow,
        ItemStatus::InProgress => Color::Cyan,
        ItemStatus::Missing | ItemStatus::Unknown => Color::DarkGray,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CodexEvent;

    #[test]
    fn renders_command_completion_as_append_only_run_block() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"Finished\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Ran cargo check"));
        assert!(rendered.contains("  └ Finished"));
        assert!(!rendered.contains("exit=0"));
        assert!(!rendered.contains("| Finished"));
    }

    #[test]
    fn skips_progress_only_command_rows() {
        let event = CodexEvent::parse(
            r#"{"type":"item.started","item":{"id":"item_0","type":"command_execution","command":"cargo check","status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn skips_progress_command_rows_with_blank_output() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"","status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn renders_completed_command_without_output_once() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"git status --short","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Ran git status --short"));
        assert!(rendered.contains("  └ no output"));
    }

    #[test]
    fn skips_empty_web_search_rows() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_0","type":"web_search","query":"","status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn skips_progress_web_search_without_query() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_0","type":"web_search","status":"in_progress"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.is_empty());
    }

    #[test]
    fn renders_web_search_with_query() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"web_search","query":"clap derive Parser docs","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Searched clap derive Parser docs"));
    }

    #[test]
    fn renders_agent_message_as_codex_block() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"**Implemented** Phase 6\n\nChanged:\n- Makefile"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("────────────────"));
        assert!(rendered.contains("• Codex"));
        assert!(rendered.contains("  Implemented Phase 6"));
        assert!(rendered.contains("  Changed:"));
        assert!(rendered.contains("Makefile"));
        assert!(!rendered.contains("**Implemented**"));
        assert!(!rendered.contains("begin"));
        assert!(!rendered.contains("end lines="));
        assert!(!rendered.contains("| Implemented"));
    }

    #[test]
    fn renders_todo_list_as_plan_block() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_1","type":"todo_list","items":[{"text":"Inspect","completed":true},{"text":"Patch","completed":false}]}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Updated Plan"));
        assert!(rendered.contains("  ✓ Inspect"));
        assert!(rendered.contains("  □ Patch"));
        assert!(!rendered.contains("-- checklist"));
    }

    #[test]
    fn renders_unknown_item_from_payload_type() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"new_tool_call","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("• Event new_tool_call"));
    }

    #[test]
    fn renders_malformed_known_item_as_warning() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(
            rendered.contains(
                "• Warning command_execution malformed: command_execution missing command"
            )
        );
    }

    #[test]
    fn colors_action_and_keeps_child_output_readable() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"Finished\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer { color: true }.render_to_string(&event);

        assert!(rendered.contains("\x1b[32m• "));
        assert!(rendered.contains("\x1b[1;32mRan"));
        assert!(rendered.contains("\x1b[90mFinished"));
        assert!(!rendered.contains("\x1b[2;90mFinished"));
    }

    #[test]
    fn colors_completed_plan_items_as_dim_struck_through() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_1","type":"todo_list","items":[{"text":"Inspect","completed":true},{"text":"Patch","completed":false}]}}"#,
        )
        .unwrap();

        let rendered = Renderer { color: true }.render_to_string(&event);

        assert!(rendered.contains("\x1b[2;9;90mInspect"));
        assert!(rendered.contains("□ "));
        assert!(!rendered.contains("\x1b[2;9;37mPatch"));
    }

    #[test]
    fn renders_representative_unicode_transcript_shape() {
        let events = [
            r#"{"type":"item.completed","item":{"id":"cmd","type":"command_execution","command":"make check","aggregated_output":"cargo fmt --all --check\ncargo clippy --all-targets --all-features -- -D warnings\ncargo test --all-features\nFinished tests\ncargo build --all-targets --all-features\nFinished build\n","exit_code":0,"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"todo","type":"todo_list","items":[{"text":"Make target setup preflight non-mutating before Git init and skill install","completed":true},{"text":"Fix repo ignore drift and run full verification","completed":false}],"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"file","type":"file_change","changes":[{"path":".gitignore","kind":"update"}],"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"bad","type":"command_execution","status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"msg","type":"agent_message","text":"Implemented all five fixes.\n\nVerification: make check passed."}}"#,
        ];
        let rendered = events
            .into_iter()
            .map(|event| {
                Renderer::without_color()
                    .render_to_string(&CodexEvent::parse(event).expect("event"))
            })
            .collect::<String>();

        assert!(rendered.contains("• Ran make check"));
        assert!(rendered.contains("    … +2 lines hidden"));
        assert!(rendered.contains("• Updated Plan"));
        assert!(rendered.contains("  ✓ Make target setup preflight"));
        assert!(rendered.contains("  □ Fix repo ignore drift"));
        assert!(rendered.contains("• Edited .gitignore"));
        assert!(rendered.contains("  └ ~ .gitignore"));
        assert!(rendered.contains("• Warning command_execution malformed"));
        assert!(rendered.contains("• Codex"));
        assert!(rendered.contains("  Verification: make check passed."));
    }
}
