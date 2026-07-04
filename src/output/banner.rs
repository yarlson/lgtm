use std::path::Path;

use crate::output::{
    options::{Charset, RenderOptions},
    style::{Color, Line, Span, Style, render_lines},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BannerMode {
    Plan,
    Run,
    Shape,
}

impl BannerMode {
    fn label(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Run => "run",
            Self::Shape => "shape",
        }
    }
}

pub(crate) struct Banner<'a> {
    pub(crate) mode: BannerMode,
    pub(crate) root: &'a Path,
    pub(crate) codex_bin: &'a str,
    pub(crate) execution: &'a str,
}

pub(crate) fn render(banner: Banner<'_>, options: &RenderOptions) -> String {
    let directory = display_path(banner.root);
    let rows = [
        ("mode:", banner.mode.label().to_string()),
        ("directory:", directory),
        ("codex:", format!("{} app-server", banner.codex_bin)),
        ("execution:", banner.execution.to_string()),
    ];
    let width = banner_width(&rows);
    let border = Border::for_charset(options.charset);
    let rule = border.horizontal.repeat(width + 2);

    let mut lines = vec![
        Line::new(vec![Span::styled(
            format!("{}{rule}{}", border.top_left, border.top_right),
            Style::fg(Color::DarkGray),
        )]),
        banner_title(width, border),
        Line::new(vec![
            Span::styled(border.vertical, Style::fg(Color::DarkGray)),
            Span::raw(" ".repeat(width + 2)),
            Span::styled(border.vertical, Style::fg(Color::DarkGray)),
        ]),
    ];

    for (label, value) in rows {
        lines.push(banner_row(width, border, label, &value));
    }

    lines.extend([
        Line::new(vec![Span::styled(
            format!("{}{rule}{}", border.bottom_left, border.bottom_right),
            Style::fg(Color::DarkGray),
        )]),
        Line::blank(),
    ]);

    render_lines(lines, options)
}

#[derive(Clone, Copy)]
struct Border {
    top_left: &'static str,
    top_right: &'static str,
    bottom_left: &'static str,
    bottom_right: &'static str,
    horizontal: &'static str,
    vertical: &'static str,
}

impl Border {
    fn for_charset(charset: Charset) -> Self {
        match charset {
            Charset::Unicode => Self {
                top_left: "╭",
                top_right: "╮",
                bottom_left: "╰",
                bottom_right: "╯",
                horizontal: "─",
                vertical: "│",
            },
            Charset::Ascii => Self {
                top_left: "+",
                top_right: "+",
                bottom_left: "+",
                bottom_right: "+",
                horizontal: "-",
                vertical: "|",
            },
        }
    }
}

fn banner_title(width: usize, border: Border) -> Line {
    let left = " >_ ";
    let name = "lgtm";
    let version = format!(" (v{})", env!("CARGO_PKG_VERSION"));
    let used = left.len() + name.len() + version.len();
    let padding = width + 2 - used;

    Line::new(vec![
        Span::styled(border.vertical, Style::fg(Color::DarkGray)),
        Span::styled(left, Style::fg(Color::DarkGray)),
        Span::styled(name, Style::fg(Color::Cyan).bold()),
        Span::styled(version, Style::fg(Color::DarkGray)),
        Span::raw(" ".repeat(padding)),
        Span::styled(border.vertical, Style::fg(Color::DarkGray)),
    ])
}

fn banner_row(width: usize, border: Border, label: &str, value: &str) -> Line {
    let label_column = 12;
    let left = format!("  {label:<label_column$} ");
    let used = left.len() + value.len();
    let padding = width + 2 - used;
    let value_style = if label == "execution:" {
        Style::fg(Color::Magenta).bold()
    } else {
        Style::fg(Color::Gray).bold()
    };

    Line::new(vec![
        Span::styled(border.vertical, Style::fg(Color::DarkGray)),
        Span::styled(left, Style::fg(Color::DarkGray)),
        Span::styled(value, value_style),
        Span::raw(" ".repeat(padding)),
        Span::styled(border.vertical, Style::fg(Color::DarkGray)),
    ])
}

fn banner_width(rows: &[(&str, String)]) -> usize {
    let title_width = format!(" >_ lgtm (v{})", env!("CARGO_PKG_VERSION")).len();
    let row_width = rows
        .iter()
        .map(|(label, value)| format!("  {label:<12} {value}").len())
        .max()
        .unwrap_or_default();
    title_width.max(row_width).max(46)
}

fn display_path(path: &Path) -> String {
    let display = path.display().to_string();
    let Some(home) = std::env::var_os("HOME") else {
        return display;
    };
    let home = Path::new(&home);
    match path.strip_prefix(home) {
        Ok(relative) if relative.as_os_str().is_empty() => "~".to_string(),
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => display,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::output::options::{ColorMode, RenderOptions};

    #[test]
    fn renders_banner_content_without_color() {
        let rendered = render(
            Banner {
                mode: BannerMode::Run,
                root: Path::new("/repo"),
                codex_bin: "codex",
                execution: "host YOLO",
            },
            &RenderOptions {
                color_mode: ColorMode::Never,
                ..RenderOptions::default()
            },
        );

        assert!(rendered.contains(">_ lgtm"));
        assert!(rendered.contains("mode:        run"));
        assert!(rendered.contains("directory:   /repo"));
        assert!(rendered.contains("codex:       codex app-server"));
        assert!(rendered.contains("execution:   host YOLO"));
    }

    #[test]
    fn renders_ascii_border_in_ascii_mode() {
        let rendered = render(
            Banner {
                mode: BannerMode::Plan,
                root: Path::new("/repo"),
                codex_bin: "codex",
                execution: "Apple Container",
            },
            &RenderOptions {
                color_mode: ColorMode::Never,
                charset: Charset::Ascii,
                ..RenderOptions::default()
            },
        );

        assert!(
            rendered
                .lines()
                .next()
                .is_some_and(|line| line.starts_with('+'))
        );
        assert!(!rendered.contains('╭'));
        assert!(!rendered.contains('│'));
    }
}
