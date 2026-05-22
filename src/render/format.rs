use chrono::Local;

use crate::events::EventKind;
use crate::events::ItemStatus;
use crate::terminal::Color;
use crate::terminal::Emphasis;
use crate::terminal::Line;
use crate::terminal::Span;
use crate::terminal::Text;
use crate::terminal::style;

pub(super) fn output_summary(output: Option<&str>) -> Option<String> {
    let output = output?;
    if output.trim().is_empty() {
        return None;
    }
    Some(format!("lines={} hidden=true raw=true", line_count(output)))
}

pub(super) fn line_count(body: &str) -> usize {
    body.lines().count()
}

pub(super) fn exit_field(exit_code: Option<i64>) -> String {
    exit_code
        .map(|code| format!(" exit={code}"))
        .unwrap_or_default()
}

pub(super) fn command_status_color(status: ItemStatus) -> Color {
    match status {
        ItemStatus::Completed => Color::Green,
        ItemStatus::Failed => Color::Red,
        ItemStatus::Declined => Color::Yellow,
        ItemStatus::InProgress => Color::Cyan,
        _ => Color::DarkGray,
    }
}

pub(super) fn event_status(event_kind: EventKind, item_status: ItemStatus) -> &'static str {
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

pub(super) fn event_status_color(event_kind: EventKind, item_status: ItemStatus) -> Color {
    match event_status(event_kind, item_status) {
        "ok" => Color::Green,
        "fail" => Color::Red,
        "skip" => Color::Yellow,
        "run" => Color::Cyan,
        _ => Color::DarkGray,
    }
}

pub(super) fn single(
    label: &'static str,
    label_color: Color,
    category: &'static str,
    message: impl Into<String>,
) -> Text {
    text(vec![row(
        label,
        label_color,
        category,
        Color::DarkGray,
        message,
    )])
}

pub(super) fn row(
    label: &'static str,
    label_color: Color,
    category: &'static str,
    category_color: Color,
    message: impl Into<String>,
) -> Line {
    Line::from(vec![
        Span::styled(
            format!("{} ", Local::now().format("%H:%M:%S%.3f")),
            style(Color::LightCyan, Emphasis::Plain),
        ),
        Span::styled(format!("{label:<4}"), style(label_color, Emphasis::Bold)),
        Span::raw(" "),
        Span::styled(
            format!("{category:<8}"),
            style(category_color, Emphasis::Plain),
        ),
        Span::raw(" "),
        Span::styled(message.into(), style(Color::Gray, Emphasis::Dim)),
    ])
}

pub(super) fn text(lines: Vec<Line>) -> Text {
    Text::from(lines)
}
