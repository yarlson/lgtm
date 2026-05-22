use termimad::MadSkin;

mod format;

use crate::events::CodexEvent;
use crate::events::CodexItem;
use crate::events::EventKind;
use crate::events::EventPayload;
use crate::events::FileChange;
use crate::events::ItemPayload;
use crate::events::ItemStatus;
use crate::events::TodoItem;
use crate::events::Usage;
use crate::render::format::command_status_color;
use crate::render::format::event_status;
use crate::render::format::event_status_color;
use crate::render::format::exit_field;
use crate::render::format::line_count;
use crate::render::format::output_summary;
use crate::render::format::row;
use crate::render::format::single;
use crate::render::format::text;
use crate::terminal::Color;
use crate::terminal::Emphasis;
use crate::terminal::Line;
use crate::terminal::Span;
use crate::terminal::Text;
use crate::terminal::style;
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
            row(
                "run",
                Color::Cyan,
                "phase",
                Color::LightBlue,
                format!("phase={phase:02} pass={action} title=\"{title}\""),
            ),
            row("info", Color::DarkGray, "log", Color::Cyan, log_path),
        ]));
    }

    pub fn system(&self, message: impl Into<String>) {
        self.emit(single("info", Color::DarkGray, "snap-rs", message));
    }

    pub fn sleep(&self, seconds: u64, next_phase: u32) {
        self.emit(single(
            "wait",
            Color::DarkGray,
            "snap-rs",
            format!("{seconds}s before Phase {next_phase}"),
        ));
    }

    pub fn raw_parse_error(&self, raw_line: &str, error: &serde_json::Error) {
        self.emit(text(vec![
            row(
                "fail",
                Color::Red,
                "json",
                Color::Red,
                format!("parse_error=\"{error}\""),
            ),
            row(
                "warn",
                Color::Yellow,
                "json",
                Color::DarkGray,
                raw_line.trim_end(),
            ),
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
            single("run", Color::Cyan, "thread", format!("thread {thread_id}"))
        }
        EventPayload::TurnStarted => single("run", Color::Cyan, "turn", "begin"),
        EventPayload::TurnCompleted { usage } => render_turn_completed(*usage),
        EventPayload::TurnFailed { message } => single(
            "fail",
            Color::Red,
            "turn",
            format!(
                "error=\"{}\"",
                message.as_deref().unwrap_or("unknown error")
            ),
        ),
        EventPayload::Error { message } => single(
            "fail",
            Color::Red,
            "codex",
            message.as_deref().unwrap_or("unknown error"),
        ),
        EventPayload::Item { item } => render_item(event.kind, item, color),
        EventPayload::Malformed { reason } => single("warn", Color::Yellow, "json", reason),
        EventPayload::Unknown => single("..", Color::DarkGray, "event", event.event_type.clone()),
    }
}

fn render_turn_completed(usage: Usage) -> Text {
    single(
        "ok",
        Color::Green,
        "turn",
        format!(
            "tokens input={} cached={} output={} reasoning={}",
            usage.input_tokens,
            usage.cached_input_tokens,
            usage.output_tokens,
            usage.reasoning_output_tokens
        ),
    )
}

fn render_item(event_kind: EventKind, item: &CodexItem, color: bool) -> Text {
    match &item.payload {
        ItemPayload::AgentMessage { text } => render_agent_message(text, color),
        ItemPayload::Reasoning { text } => render_block("reason", Color::DarkGray, text),
        ItemPayload::CommandExecution {
            command,
            output,
            exit_code,
        } => render_command(
            event_kind,
            item.status,
            command,
            output.as_deref(),
            *exit_code,
        ),
        ItemPayload::FileChange { changes } => render_file_change(event_kind, item.status, changes),
        ItemPayload::McpToolCall {
            server,
            tool,
            error_message,
        } => render_mcp(
            event_kind,
            item.status,
            server,
            tool,
            error_message.as_deref(),
        ),
        ItemPayload::CollabToolCall {
            tool,
            receiver_count,
        } => render_collab(event_kind, item.status, tool, *receiver_count),
        ItemPayload::WebSearch { query } => single(
            event_status(event_kind, item.status),
            event_status_color(event_kind, item.status),
            "web",
            query,
        ),
        ItemPayload::TodoList { items } => render_todos(event_kind, item.status, items),
        ItemPayload::Error { message } => single(
            "fail",
            Color::Red,
            "codex",
            message.as_deref().unwrap_or("unknown error"),
        ),
        ItemPayload::Malformed { item_type, reason } => single(
            "warn",
            Color::Yellow,
            "item",
            format!("{item_type} malformed: {reason}"),
        ),
        ItemPayload::Unknown { item_type } => single(
            event_status(event_kind, item.status),
            Color::DarkGray,
            "item",
            item_type,
        ),
    }
}

fn render_agent_message(message: &str, color: bool) -> Text {
    let mut lines = Vec::new();
    lines.push(row(
        "msg",
        Color::LightMagenta,
        "codex",
        Color::LightMagenta,
        "begin",
    ));
    lines.extend(markdown_lines(message, color));
    lines.push(row(
        "msg",
        Color::LightMagenta,
        "codex",
        Color::LightMagenta,
        format!("end lines={}", line_count(message)),
    ));
    text(lines)
}

fn render_command(
    event_kind: EventKind,
    status: ItemStatus,
    command: &str,
    output: Option<&str>,
    exit_code: Option<i64>,
) -> Text {
    let mut message = format!("exec {command}{}", exit_field(exit_code));
    if let Some(summary) = output_summary(output) {
        message.push(' ');
        message.push_str(&summary);
    }

    text(vec![row(
        event_status(event_kind, status),
        command_status_color(status),
        "tool",
        Color::Cyan,
        message,
    )])
}

fn render_file_change(event_kind: EventKind, status: ItemStatus, changes: &[FileChange]) -> Text {
    let mut lines = vec![row(
        event_status(event_kind, status),
        command_status_color(status),
        "files",
        Color::Yellow,
        format!("patch files={}", changes.len()),
    )];

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
        lines.push(row(
            "..",
            color,
            "files",
            Color::Yellow,
            format!("{marker} {}", change.path),
        ));
    }

    text(lines)
}

fn render_mcp(
    event_kind: EventKind,
    status: ItemStatus,
    server: &str,
    tool: &str,
    error_message: Option<&str>,
) -> Text {
    let mut lines = vec![row(
        event_status(event_kind, status),
        command_status_color(status),
        "mcp",
        Color::LightBlue,
        format!("{server}/{tool}"),
    )];
    if let Some(error) = error_message {
        lines.push(row("fail", Color::Red, "mcp", Color::LightBlue, error));
    }
    text(lines)
}

fn render_collab(
    event_kind: EventKind,
    status: ItemStatus,
    tool: &str,
    receiver_count: usize,
) -> Text {
    single(
        event_status(event_kind, status),
        event_status_color(event_kind, status),
        "collab",
        format!("{tool} receiver_threads={receiver_count}"),
    )
}

fn render_todos(event_kind: EventKind, status: ItemStatus, todos: &[TodoItem]) -> Text {
    let mut lines = vec![row(
        event_status(event_kind, status),
        event_status_color(event_kind, status),
        "todo",
        Color::Yellow,
        format!("items={}", todos.len()),
    )];
    for todo in todos {
        let (status, color) = if todo.completed {
            ("ok", Color::Green)
        } else {
            ("..", Color::DarkGray)
        };
        lines.push(row(
            status,
            color,
            "todo",
            Color::Yellow,
            todo.text.as_str(),
        ));
    }
    text(lines)
}

fn render_block(label: &'static str, color: Color, body: &str) -> Text {
    let mut lines = vec![row("msg", color, label, color, "begin")];
    lines.extend(block_lines(body, color));
    lines.push(row(
        "msg",
        color,
        label,
        color,
        format!("end lines={}", line_count(body)),
    ));
    text(lines)
}

fn block_lines(body: &str, color: Color) -> Vec<Line> {
    if body.trim().is_empty() {
        return Vec::new();
    }
    body.lines()
        .map(|line| {
            Line::from(vec![Span::styled(
                line.to_string(),
                style(color, Emphasis::Plain),
            )])
        })
        .collect()
}

fn markdown_lines(markdown: &str, color: bool) -> Vec<Line> {
    if markdown.trim().is_empty() {
        return Vec::new();
    }

    let skin = if color {
        MadSkin::default_dark()
    } else {
        MadSkin::no_style()
    };

    skin.term_text(markdown)
        .to_string()
        .lines()
        .map(|line| Line::from(vec![Span::raw(line.to_string())]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::CodexEvent;

    #[test]
    fn renders_command_completion_as_collapsed_tool_row() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"Finished\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("ok   tool"));
        assert!(rendered.contains("exec cargo check exit=0 lines=1 hidden=true raw=true"));
        assert!(!rendered.contains("\nFinished"));
        assert!(!rendered.contains("| Finished"));
    }

    #[test]
    fn renders_agent_message_markdown_at_left_edge() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Implemented Phase 6\n\nChanged:\n- Makefile"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("msg  codex"));
        assert!(rendered.contains("begin"));
        assert!(rendered.contains("end lines=4"));
        assert!(rendered.lines().any(|line| line == "Implemented Phase 6"));
        assert!(!rendered.contains("    Implemented Phase 6"));
        assert!(!rendered.contains("| Implemented"));
    }

    #[test]
    fn renders_todo_list_as_append_only_rows() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_1","type":"todo_list","items":[{"text":"Inspect","completed":true},{"text":"Patch","completed":false}]}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("..   todo     items=2"));
        assert!(rendered.contains("ok   todo     Inspect"));
        assert!(rendered.contains("..   todo     Patch"));
        assert!(!rendered.contains("-- checklist"));
    }

    #[test]
    fn renders_unknown_item_from_payload_type() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"new_tool_call","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("ok   item     new_tool_call"));
    }

    #[test]
    fn renders_malformed_known_item_as_warning() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains(
            "warn item     command_execution malformed: command_execution missing command"
        ));
    }

    #[test]
    fn colors_time_and_dims_detail_column() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"Finished\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer { color: true }.render_to_string(&event);

        assert!(rendered.contains("\x1b[96m"));
        assert!(rendered.contains("\x1b[2;37mexec cargo check"));
    }
}
