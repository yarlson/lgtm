use crate::terminal::Color;
use crate::terminal::Emphasis;
use crate::terminal::Line;
use crate::terminal::Span;
use crate::terminal::Text;
use crate::terminal::decorated_style;
use crate::terminal::style;

const PREVIEW_LINES: usize = 4;

pub(super) fn block(action: &'static str, color: Color, message: impl Into<String>) -> Text {
    text(vec![header(action, color, message)])
}

pub(super) fn header(action: &'static str, color: Color, message: impl Into<String>) -> Line {
    header_with_marker("•", action, color, message)
}

pub(super) fn header_with_marker(
    marker: impl Into<String>,
    action: &'static str,
    color: Color,
    message: impl Into<String>,
) -> Line {
    let message = message.into();
    let mut spans = vec![
        Span::styled(marker.into(), style(Color::Green, Emphasis::Plain)),
        Span::raw(" "),
        Span::styled(action, style(color, Emphasis::Bold)),
    ];
    if !message.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(message, style(Color::Gray, Emphasis::Plain)));
    }
    Line::from(spans)
}

pub(super) fn child(message: impl Into<String>) -> Line {
    Line::from(vec![
        Span::raw("  "),
        Span::styled("└ ", style(Color::DarkGray, Emphasis::Plain)),
        Span::styled(message.into(), style(Color::DarkGray, Emphasis::Plain)),
    ])
}

pub(super) fn continuation(message: impl Into<String>) -> Line {
    Line::from(vec![
        Span::raw("    "),
        Span::styled(message.into(), style(Color::DarkGray, Emphasis::Plain)),
    ])
}

pub(super) fn raw_body_line(message: impl Into<String>) -> Line {
    Line::from(vec![Span::raw("  "), Span::raw(message)])
}

pub(super) fn checklist_line(completed: bool, message: impl Into<String>) -> Line {
    let marker = if completed { "✓ " } else { "□ " };
    let color = if completed {
        Color::Green
    } else {
        Color::DarkGray
    };
    let text_color = if completed {
        Color::DarkGray
    } else {
        Color::Gray
    };

    Line::from(vec![
        Span::raw("  "),
        Span::styled(marker, style(color, Emphasis::Plain)),
        Span::styled(
            message.into(),
            decorated_style(text_color, false, completed, completed),
        ),
    ])
}

pub(super) fn file_line(marker: &'static str, color: Color, message: impl Into<String>) -> Line {
    Line::from(vec![
        Span::raw("  "),
        Span::styled("└ ", style(Color::DarkGray, Emphasis::Plain)),
        Span::styled(marker, style(color, Emphasis::Plain)),
        Span::raw(" "),
        Span::styled(message.into(), style(color, Emphasis::Plain)),
    ])
}

pub(super) fn separator() -> Line {
    Line::from(vec![Span::styled(
        "────────────────────────────────────────────────────────────────".to_string(),
        style(Color::DarkGray, Emphasis::Dim),
    )])
}

pub(super) fn blank() -> Line {
    Line::from(Vec::new())
}

pub(super) fn output_lines(output: Option<&str>) -> Vec<Line> {
    let Some(output) = output else {
        return vec![child("no output")];
    };
    if output.trim().is_empty() {
        return vec![child("no output")];
    }

    let mut lines = Vec::new();
    let output_lines: Vec<&str> = output.lines().collect();
    for (index, line) in output_lines.iter().take(PREVIEW_LINES).enumerate() {
        if index == 0 {
            lines.push(child(line.trim_end()));
        } else {
            lines.push(continuation(line.trim_end()));
        }
    }

    let hidden = output_lines.len().saturating_sub(PREVIEW_LINES);
    if hidden > 0 {
        lines.push(continuation(format!("… +{hidden} lines hidden")));
    }
    lines
}

pub(super) fn text(lines: Vec<Line>) -> Text {
    Text::from(lines)
}
