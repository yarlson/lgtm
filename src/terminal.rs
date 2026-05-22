#[derive(Debug, Clone)]
pub(crate) struct Text {
    pub(crate) lines: Vec<Line>,
}

#[derive(Debug, Clone)]
pub(crate) struct Line {
    pub(crate) spans: Vec<Span>,
}

#[derive(Debug, Clone)]
pub(crate) struct Span {
    pub(crate) content: String,
    pub(crate) style: Style,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Style {
    fg: Option<Color>,
    emphasis: Emphasis,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Emphasis {
    Plain,
    Bold,
    Dim,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Color {
    Red,
    Green,
    Yellow,
    Cyan,
    Gray,
    DarkGray,
    LightBlue,
    LightMagenta,
    LightCyan,
}

impl Text {
    pub(crate) fn from(lines: Vec<Line>) -> Self {
        Self { lines }
    }
}

impl Line {
    pub(crate) fn from(spans: Vec<Span>) -> Self {
        Self { spans }
    }
}

impl Span {
    pub(crate) fn raw(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: Style::plain(),
        }
    }

    pub(crate) fn styled(content: impl Into<String>, style: Style) -> Self {
        Self {
            content: content.into(),
            style,
        }
    }
}

impl Style {
    fn plain() -> Self {
        Self {
            fg: None,
            emphasis: Emphasis::Plain,
        }
    }
}

pub(crate) fn style(color: Color, emphasis: Emphasis) -> Style {
    Style {
        fg: Some(color),
        emphasis,
    }
}

pub(crate) fn text_to_string(text: Text, color: bool) -> String {
    let mut out = String::new();
    for line in text.lines {
        for span in line.spans {
            if color {
                out.push_str(&ansi_start(span.style));
            }
            out.push_str(&span.content);
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
    match style.emphasis {
        Emphasis::Plain => {}
        Emphasis::Bold => codes.push("1"),
        Emphasis::Dim => codes.push("2"),
    }
    if let Some(color) = style.fg {
        codes.push(ansi_color(color));
    }
    if codes.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

fn ansi_color(color: Color) -> &'static str {
    match color {
        Color::Red => "31",
        Color::Green => "32",
        Color::Yellow => "33",
        Color::Cyan => "36",
        Color::Gray => "37",
        Color::DarkGray => "90",
        Color::LightBlue => "94",
        Color::LightMagenta => "95",
        Color::LightCyan => "96",
    }
}
