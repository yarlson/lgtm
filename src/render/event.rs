use crate::events::CodexEvent;
use crate::events::CodexItem;
use crate::events::EventPayload;
use crate::events::FileChange;
use crate::events::ItemPayload;
use crate::events::ItemStatus;
use crate::events::TodoItem;
use crate::events::Usage;
use crate::render::format;
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

pub(super) fn render_event(event: &CodexEvent, color: bool) -> Text {
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
    super::is_progress_status(status)
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

fn markdown_lines(markdown: &str, color: bool) -> Vec<crate::terminal::Line> {
    let rendered = super::markdown::markdown_to_string(markdown, color);
    if rendered.is_empty() {
        return Vec::new();
    }

    rendered
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
