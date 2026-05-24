use crate::output::{
    options::{MarkdownMode, RenderOptions, color_enabled},
    style::{Line, Span},
};

pub(crate) fn markdown_lines(markdown: &str, options: &RenderOptions) -> Vec<Line> {
    let rendered = match options.markdown {
        MarkdownMode::Basic => render_term_markdown(markdown, color_enabled(options.color_mode)),
        MarkdownMode::Plain => markdown.to_string(),
    };

    rendered
        .trim_end_matches(['\r', '\n'])
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                Line::blank()
            } else {
                Line::new(vec![Span::raw("  "), Span::raw(line)])
            }
        })
        .collect()
}

fn render_term_markdown(markdown: &str, color: bool) -> String {
    let markdown = markdown.trim_end_matches(['\r', '\n']);
    if markdown.trim().is_empty() {
        return String::new();
    }

    let skin = if color {
        termimad::MadSkin::default_dark()
    } else {
        termimad::MadSkin::no_style()
    };

    skin.term_text(markdown)
        .to_string()
        .trim_end_matches(['\r', '\n'])
        .to_string()
}
