use crate::output::{
    MarkdownMode,
    style::{Line, Span},
};

pub(crate) fn markdown_lines(markdown: &str, mode: MarkdownMode) -> Vec<Line> {
    let rendered = match mode {
        MarkdownMode::Basic => basic_markdown(markdown),
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

fn basic_markdown(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            let line = line.trim_end();
            let line = line.strip_prefix("- ").unwrap_or(line);
            line.replace("**", "").replace("__", "").replace('`', "")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
