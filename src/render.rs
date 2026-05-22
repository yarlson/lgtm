use chrono::Local;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;
use termimad::MadSkin;

use crate::events::CodexEvent;
use crate::events::CodexItem;
use crate::events::EventKind;
use crate::events::ItemKind;
use crate::events::ItemStatus;
use crate::events::Usage;

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

    fn emit(&self, rendered: Text<'static>) {
        print!("{}", text_to_string(rendered, self.color));
    }
}

fn render_event(event: &CodexEvent, color: bool) -> Text<'static> {
    match event.kind {
        EventKind::ThreadStarted => single(
            "run",
            Color::Cyan,
            "thread",
            format!("thread {}", event.string_at("thread_id").unwrap_or("")),
        ),
        EventKind::TurnStarted => single("run", Color::Cyan, "turn", "begin"),
        EventKind::TurnCompleted => render_turn_completed(event.usage()),
        EventKind::TurnFailed => single(
            "fail",
            Color::Red,
            "turn",
            format!(
                "error=\"{}\"",
                event
                    .error_message()
                    .unwrap_or_else(|| "unknown error".to_string())
            ),
        ),
        EventKind::Error => single(
            "fail",
            Color::Red,
            "codex",
            event
                .error_message()
                .unwrap_or_else(|| "unknown error".to_string()),
        ),
        EventKind::ItemStarted | EventKind::ItemUpdated | EventKind::ItemCompleted => {
            if let Some(item) = event.item() {
                render_item(event.kind, &item, color)
            } else {
                single("..", Color::DarkGray, "event", event.event_type.clone())
            }
        }
        EventKind::Unknown => single("..", Color::DarkGray, "event", event.event_type.clone()),
    }
}

fn render_turn_completed(usage: Usage) -> Text<'static> {
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

fn render_item(event_kind: EventKind, item: &CodexItem, color: bool) -> Text<'static> {
    match item.kind {
        ItemKind::AgentMessage => render_agent_message(item.text().unwrap_or(""), color),
        ItemKind::Reasoning => render_block("reason", Color::DarkGray, item.text().unwrap_or("")),
        ItemKind::CommandExecution => render_command(event_kind, item),
        ItemKind::FileChange => render_file_change(event_kind, item),
        ItemKind::McpToolCall => render_mcp(event_kind, item),
        ItemKind::CollabToolCall => render_collab(event_kind, item),
        ItemKind::WebSearch => single(
            event_status(event_kind, item.status),
            event_status_color(event_kind, item.status),
            "web",
            item.string_at("query").unwrap_or(""),
        ),
        ItemKind::TodoList => render_todos(event_kind, item),
        ItemKind::Error => single(
            "fail",
            Color::Red,
            "codex",
            item.error_message().unwrap_or("unknown error"),
        ),
        ItemKind::Unknown => single(
            event_status(event_kind, item.status),
            Color::DarkGray,
            "item",
            item.item_type.clone(),
        ),
    }
}

fn render_agent_message(message: &str, color: bool) -> Text<'static> {
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

fn render_command(event_kind: EventKind, item: &CodexItem) -> Text<'static> {
    let command = item.string_at("command").unwrap_or("");
    let mut message = format!("exec {command}{}", exit_field(item));
    if let Some(summary) = output_summary(item.command_output()) {
        message.push(' ');
        message.push_str(&summary);
    }

    text(vec![row(
        event_status(event_kind, item.status),
        command_status_color(item.status),
        "tool",
        Color::Cyan,
        message,
    )])
}

fn render_file_change(event_kind: EventKind, item: &CodexItem) -> Text<'static> {
    let changes = item.changes();
    let mut lines = vec![row(
        event_status(event_kind, item.status),
        command_status_color(item.status),
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

fn render_mcp(event_kind: EventKind, item: &CodexItem) -> Text<'static> {
    let mut lines = vec![row(
        event_status(event_kind, item.status),
        command_status_color(item.status),
        "mcp",
        Color::LightBlue,
        format!(
            "{}/{}",
            item.string_at("server").unwrap_or(""),
            item.string_at("tool").unwrap_or("")
        ),
    )];
    if let Some(error) = item.error_message() {
        lines.push(row("fail", Color::Red, "mcp", Color::LightBlue, error));
    }
    text(lines)
}

fn render_collab(event_kind: EventKind, item: &CodexItem) -> Text<'static> {
    single(
        event_status(event_kind, item.status),
        event_status_color(event_kind, item.status),
        "collab",
        format!(
            "{} receiver_threads={}",
            item.string_at("tool").unwrap_or("unknown"),
            item.receiver_count()
        ),
    )
}

fn render_todos(event_kind: EventKind, item: &CodexItem) -> Text<'static> {
    let todos = item.todos();
    let mut lines = vec![row(
        event_status(event_kind, item.status),
        event_status_color(event_kind, item.status),
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
        lines.push(row(status, color, "todo", Color::Yellow, todo.text));
    }
    text(lines)
}

fn render_block(label: &'static str, color: Color, body: &str) -> Text<'static> {
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

fn block_lines(body: &str, color: Color) -> Vec<Line<'static>> {
    if body.trim().is_empty() {
        return Vec::new();
    }
    body.lines()
        .map(|line| {
            Line::from(vec![Span::styled(
                line.to_string(),
                style(color, Modifier::empty()),
            )])
        })
        .collect()
}

fn markdown_lines(markdown: &str, color: bool) -> Vec<Line<'static>> {
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

fn output_summary(output: Option<&str>) -> Option<String> {
    let output = output?;
    if output.trim().is_empty() {
        return None;
    }
    Some(format!("lines={} hidden=true raw=true", line_count(output)))
}

fn line_count(body: &str) -> usize {
    body.lines().count()
}

fn exit_field(item: &CodexItem) -> String {
    item.exit_code()
        .map(|code| format!(" exit={code}"))
        .unwrap_or_default()
}

fn command_status_color(status: ItemStatus) -> Color {
    match status {
        ItemStatus::Completed => Color::Green,
        ItemStatus::Failed => Color::Red,
        ItemStatus::Declined => Color::Yellow,
        ItemStatus::InProgress => Color::Cyan,
        _ => Color::DarkGray,
    }
}

fn event_status(event_kind: EventKind, item_status: ItemStatus) -> &'static str {
    match item_status {
        ItemStatus::Completed => "ok",
        ItemStatus::Failed => "fail",
        ItemStatus::Declined => "skip",
        ItemStatus::InProgress => "run",
        ItemStatus::Missing | ItemStatus::Unknown => match event_kind {
            EventKind::ItemStarted => "run",
            EventKind::ItemCompleted => "ok",
            EventKind::ItemUpdated => "..",
            _ => "..",
        },
    }
}

fn event_status_color(event_kind: EventKind, item_status: ItemStatus) -> Color {
    match event_status(event_kind, item_status) {
        "ok" => Color::Green,
        "fail" => Color::Red,
        "skip" => Color::Yellow,
        "run" => Color::Cyan,
        _ => Color::DarkGray,
    }
}

fn single(
    label: &'static str,
    label_color: Color,
    category: &'static str,
    message: impl Into<String>,
) -> Text<'static> {
    text(vec![row(
        label,
        label_color,
        category,
        Color::DarkGray,
        message,
    )])
}

fn row(
    label: &'static str,
    label_color: Color,
    category: &'static str,
    category_color: Color,
    message: impl Into<String>,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{} ", Local::now().format("%H:%M:%S%.3f")),
            style(Color::LightCyan, Modifier::empty()),
        ),
        Span::styled(format!("{label:<4}"), style(label_color, Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(
            format!("{category:<8}"),
            style(category_color, Modifier::empty()),
        ),
        Span::raw(" "),
        Span::styled(message.into(), style(Color::Gray, Modifier::DIM)),
    ])
}

fn text(lines: Vec<Line<'static>>) -> Text<'static> {
    Text::from(lines)
}

fn style(color: Color, modifier: Modifier) -> Style {
    Style::default().fg(color).add_modifier(modifier)
}

fn text_to_string(text: Text<'static>, color: bool) -> String {
    let mut out = String::new();
    for line in text.lines {
        for span in line.spans {
            if color {
                out.push_str(&ansi_start(span.style));
            }
            out.push_str(span.content.as_ref());
            if color {
                out.push_str("\x1b[0m");
            }
        }
        out.push('\n');
    }
    out
}

fn ansi_start(style: Style) -> String {
    let mut codes = Vec::new();
    if style.add_modifier.contains(Modifier::BOLD) {
        codes.push("1");
    }
    if style.add_modifier.contains(Modifier::ITALIC) {
        codes.push("3");
    }
    if style.add_modifier.contains(Modifier::DIM) {
        codes.push("2");
    }
    if let Some(color) = style.fg
        && let Some(code) = ansi_color(color)
    {
        codes.push(code);
    }
    if codes.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

fn ansi_color(color: Color) -> Option<&'static str> {
    match color {
        Color::Black => Some("30"),
        Color::Red => Some("31"),
        Color::Green => Some("32"),
        Color::Yellow => Some("33"),
        Color::Blue => Some("34"),
        Color::Magenta => Some("35"),
        Color::Cyan => Some("36"),
        Color::Gray | Color::White => Some("37"),
        Color::DarkGray => Some("90"),
        Color::LightRed => Some("91"),
        Color::LightGreen => Some("92"),
        Color::LightYellow => Some("93"),
        Color::LightBlue => Some("94"),
        Color::LightMagenta => Some("95"),
        Color::LightCyan => Some("96"),
        Color::Indexed(_) | Color::Rgb(_, _, _) | Color::Reset => None,
    }
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
