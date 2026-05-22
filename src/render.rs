use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::text::Text;

use crate::events::CodexEvent;
use crate::events::CodexItem;
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
            Line::from(vec![
                Span::styled(
                    format!("== Phase {phase:02}: "),
                    style(Color::LightBlue, Modifier::BOLD),
                ),
                Span::styled(title.to_string(), style(Color::White, Modifier::BOLD)),
                Span::styled(
                    format!(" [{action}]"),
                    style(Color::DarkGray, Modifier::empty()),
                ),
            ]),
            Line::from(vec![
                Span::raw("   "),
                Span::styled("log ", style(Color::DarkGray, Modifier::empty())),
                Span::styled(log_path.to_string(), style(Color::Cyan, Modifier::empty())),
            ]),
        ]));
    }

    pub fn system(&self, message: impl Into<String>) {
        self.emit(single("log", Color::DarkGray, message.into()));
    }

    pub fn sleep(&self, seconds: u64, next_phase: u32) {
        self.emit(single(
            "wait",
            Color::DarkGray,
            format!("{seconds}s before Phase {next_phase}"),
        ));
    }

    pub fn raw_parse_error(&self, raw_line: &str, error: &serde_json::Error) {
        self.emit(text(vec![
            row(
                "fail",
                Color::Red,
                format!("could not parse Codex event: {error}"),
            ),
            Line::from(vec![
                Span::styled("    raw ", style(Color::DarkGray, Modifier::empty())),
                Span::raw(raw_line.trim_end().to_string()),
            ]),
        ]));
    }

    pub fn event(&self, event: &CodexEvent) {
        self.emit(render_event(event));
    }

    #[cfg(test)]
    pub fn render_to_string(&self, event: &CodexEvent) -> String {
        text_to_string(render_event(event), self.color)
    }

    fn emit(&self, rendered: Text<'static>) {
        print!("{}", text_to_string(rendered, self.color));
    }
}

fn render_event(event: &CodexEvent) -> Text<'static> {
    match event.event_type.as_str() {
        "thread.started" => single(
            "run",
            Color::Cyan,
            format!("thread {}", event.string_at("thread_id").unwrap_or("")),
        ),
        "turn.started" => single("run", Color::Cyan, "turn"),
        "turn.completed" => render_turn_completed(event.usage()),
        "turn.failed" => single(
            "fail",
            Color::Red,
            format!(
                "turn {}",
                event
                    .error_message()
                    .unwrap_or_else(|| "unknown error".to_string())
            ),
        ),
        "error" => single(
            "fail",
            Color::Red,
            event
                .error_message()
                .unwrap_or_else(|| "unknown error".to_string()),
        ),
        "item.started" | "item.updated" | "item.completed" => {
            if let Some(item) = event.item() {
                render_item(event.event_type.as_str(), &item)
            } else {
                single("..", Color::DarkGray, event.event_type.clone())
            }
        }
        _ => single("..", Color::DarkGray, event.event_type.clone()),
    }
}

fn render_turn_completed(usage: Usage) -> Text<'static> {
    single(
        "ok",
        Color::Green,
        format!(
            "turn tokens input={} cached={} output={} reasoning={}",
            usage.input_tokens,
            usage.cached_input_tokens,
            usage.output_tokens,
            usage.reasoning_output_tokens
        ),
    )
}

fn render_item(event_type: &str, item: &CodexItem) -> Text<'static> {
    match item.item_type.as_str() {
        "agent_message" => render_agent_message(item.text().unwrap_or("")),
        "reasoning" => render_block("reasoning", Color::DarkGray, item.text().unwrap_or("")),
        "command_execution" => render_command(event_type, item),
        "file_change" => render_file_change(event_type, item),
        "mcp_tool_call" => render_mcp(event_type, item),
        "collab_tool_call" => render_collab(event_type, item),
        "web_search" => single(
            event_status(event_type, item.status()),
            event_status_color(event_type, item.status()),
            format!("web {}", item.string_at("query").unwrap_or("")),
        ),
        "todo_list" => render_todos(event_type, item),
        "error" => single(
            "fail",
            Color::Red,
            item.error_message().unwrap_or("unknown error"),
        ),
        _ => single(
            event_status(event_type, item.status()),
            Color::DarkGray,
            item.item_type.clone(),
        ),
    }
}

fn render_agent_message(message: &str) -> Text<'static> {
    let mut lines = Vec::new();
    lines.push(section("codex"));
    lines.extend(prose_lines(message, Color::White));
    text(lines)
}

fn render_command(event_type: &str, item: &CodexItem) -> Text<'static> {
    let status = item.status().unwrap_or("unknown");
    let command = item.string_at("command").unwrap_or("");
    let mut lines = vec![row(
        event_status(event_type, Some(status)),
        command_status_color(status),
        format!("exec {command}{}", exit_suffix(item)),
    )];

    if should_show_command_output(status, item.command_output()) {
        lines.extend(prose_lines(
            item.command_output().unwrap_or(""),
            Color::DarkGray,
        ));
    } else if let Some(output) = item.command_output()
        && !output.trim().is_empty()
    {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!(
                    "output {} lines hidden; raw log has full output",
                    output.lines().count()
                ),
                style(Color::DarkGray, Modifier::ITALIC),
            ),
        ]));
    }

    text(lines)
}

fn render_file_change(event_type: &str, item: &CodexItem) -> Text<'static> {
    let status = item.status().unwrap_or("unknown");
    let changes = item.changes();
    let mut lines = vec![
        section("files"),
        row(
            event_status(event_type, Some(status)),
            command_status_color(status),
            format!("patch {} files", changes.len()),
        ),
    ];

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
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(marker, style(color, Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(change.path, style(Color::DarkGray, Modifier::empty())),
        ]));
    }

    text(lines)
}

fn render_mcp(event_type: &str, item: &CodexItem) -> Text<'static> {
    let status = item.status().unwrap_or("unknown");
    let mut lines = vec![row(
        event_status(event_type, Some(status)),
        command_status_color(status),
        format!(
            "mcp {}/{}",
            item.string_at("server").unwrap_or(""),
            item.string_at("tool").unwrap_or("")
        ),
    )];
    if let Some(error) = item.error_message() {
        lines.extend(prose_lines(error, Color::Red));
    }
    text(lines)
}

fn render_collab(event_type: &str, item: &CodexItem) -> Text<'static> {
    single(
        event_status(event_type, item.status()),
        event_status_color(event_type, item.status()),
        format!(
            "collab {} ({} receiver threads)",
            item.string_at("tool").unwrap_or("unknown"),
            item.receiver_count()
        ),
    )
}

fn render_todos(event_type: &str, item: &CodexItem) -> Text<'static> {
    let todos = item.todos();
    let mut lines = vec![
        section("checklist"),
        row(
            event_status(event_type, item.status()),
            event_status_color(event_type, item.status()),
            format!("todo {} items", todos.len()),
        ),
    ];
    for todo in todos {
        let (status, color) = if todo.completed {
            ("ok", Color::Green)
        } else {
            ("..", Color::DarkGray)
        };
        lines.push(row(status, color, todo.text));
    }
    text(lines)
}

fn render_block(label: &'static str, color: Color, body: &str) -> Text<'static> {
    let mut lines = vec![section(label)];
    lines.extend(prose_lines(body, color));
    text(lines)
}

fn prose_lines(body: &str, color: Color) -> Vec<Line<'static>> {
    if body.trim().is_empty() {
        return Vec::new();
    }
    body.lines()
        .map(|line| {
            Line::from(vec![
                Span::raw("    "),
                Span::styled(line.to_string(), style(color, Modifier::empty())),
            ])
        })
        .collect()
}

fn should_show_command_output(status: &str, output: Option<&str>) -> bool {
    let Some(output) = output else {
        return false;
    };
    if output.trim().is_empty() {
        return false;
    }
    status != "completed" || output.lines().count() <= 24
}

fn exit_suffix(item: &CodexItem) -> String {
    item.exit_code()
        .map(|code| format!(" (exit {code})"))
        .unwrap_or_default()
}

fn command_status_color(status: &str) -> Color {
    match status {
        "completed" => Color::Green,
        "failed" => Color::Red,
        "declined" => Color::Yellow,
        "in_progress" => Color::Cyan,
        _ => Color::DarkGray,
    }
}

fn event_status(event_type: &str, item_status: Option<&str>) -> &'static str {
    match item_status {
        Some("completed") => "ok",
        Some("failed") => "fail",
        Some("declined") => "skip",
        Some("in_progress") => "run",
        _ if event_type.ends_with(".started") => "run",
        _ if event_type.ends_with(".completed") => "ok",
        _ if event_type.ends_with(".updated") => "..",
        _ => "..",
    }
}

fn event_status_color(event_type: &str, item_status: Option<&str>) -> Color {
    match event_status(event_type, item_status) {
        "ok" => Color::Green,
        "fail" => Color::Red,
        "skip" => Color::Yellow,
        "run" => Color::Cyan,
        _ => Color::DarkGray,
    }
}

fn single(label: &'static str, label_color: Color, message: impl Into<String>) -> Text<'static> {
    text(vec![row(label, label_color, message)])
}

fn row(label: &'static str, label_color: Color, message: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<10}"), style(label_color, Modifier::BOLD)),
        Span::raw(" "),
        Span::raw(message.into()),
    ])
}

fn section(label: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled("-- ", style(Color::DarkGray, Modifier::empty())),
        Span::styled(label, style(Color::LightBlue, Modifier::BOLD)),
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
    fn renders_command_completion_with_output() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"command_execution","command":"cargo check","aggregated_output":"Finished\n","exit_code":0,"status":"completed"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("ok"));
        assert!(rendered.contains("exec cargo check (exit 0)"));
        assert!(rendered.contains("    Finished"));
        assert!(!rendered.contains("| Finished"));
    }

    #[test]
    fn renders_agent_message_without_pipe_prefixes() {
        let event = CodexEvent::parse(
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Implemented Phase 6\nChanged:\n- Makefile"}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("-- codex"));
        assert!(rendered.contains("    Implemented Phase 6"));
        assert!(rendered.contains("    Changed:"));
        assert!(!rendered.contains("| Implemented"));
    }

    #[test]
    fn renders_todo_list_as_checklist() {
        let event = CodexEvent::parse(
            r#"{"type":"item.updated","item":{"id":"item_1","type":"todo_list","items":[{"text":"Inspect","completed":true},{"text":"Patch","completed":false}]}}"#,
        )
        .unwrap();

        let rendered = Renderer::without_color().render_to_string(&event);

        assert!(rendered.contains("-- checklist"));
        assert!(rendered.contains("ok         Inspect"));
        assert!(rendered.contains("..         Patch"));
    }
}
