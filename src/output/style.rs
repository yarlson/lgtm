use crate::output::options::{Charset, RenderOptions, color_enabled};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    spans: Vec<Span>,
}

impl Line {
    pub(crate) fn new(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    pub(crate) fn blank() -> Self {
        Self { spans: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    content: String,
    style: Style,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Style {
    fg: Option<Color>,
    bold: bool,
    dim: bool,
    strikethrough: bool,
}

impl Style {
    pub(crate) fn plain() -> Self {
        Self {
            fg: None,
            bold: false,
            dim: false,
            strikethrough: false,
        }
    }

    pub(crate) fn fg(color: Color) -> Self {
        Self {
            fg: Some(color),
            ..Self::plain()
        }
    }

    pub(crate) fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub(crate) fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub(crate) fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Color {
    Red,
    Green,
    Yellow,
    Cyan,
    Gray,
    DarkGray,
    Blue,
    Magenta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Symbol {
    Bullet,
    Branch,
    Check,
    EmptyBox,
    Ellipsis,
    Rule,
}

impl Symbol {
    pub(crate) fn render(self, charset: Charset) -> &'static str {
        match (charset, self) {
            (Charset::Unicode, Symbol::Bullet) => "•",
            (Charset::Unicode, Symbol::Branch) => "└ ",
            (Charset::Unicode, Symbol::Check) => "✓ ",
            (Charset::Unicode, Symbol::EmptyBox) => "□ ",
            (Charset::Unicode, Symbol::Ellipsis) => "…",
            (Charset::Unicode, Symbol::Rule) => {
                "────────────────────────────────────────────────────────────────"
            }
            (Charset::Ascii, Symbol::Bullet) => "*",
            (Charset::Ascii, Symbol::Branch) => "`- ",
            (Charset::Ascii, Symbol::Check) => "[x] ",
            (Charset::Ascii, Symbol::EmptyBox) => "[ ] ",
            (Charset::Ascii, Symbol::Ellipsis) => "...",
            (Charset::Ascii, Symbol::Rule) => {
                "----------------------------------------------------------------"
            }
        }
    }
}

pub(crate) fn render_lines(lines: Vec<Line>, options: &RenderOptions) -> String {
    if lines.is_empty() {
        return String::new();
    }

    let color = color_enabled(options.color_mode);
    let mut out = String::new();
    for line in lines {
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
    if style.bold {
        codes.push("1");
    }
    if style.dim {
        codes.push("2");
    }
    if style.strikethrough {
        codes.push("9");
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
        Color::Blue => "94",
        Color::Magenta => "95",
    }
}
