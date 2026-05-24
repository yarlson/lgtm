mod item;
mod markdown;
mod options;
mod renderer;
pub(crate) mod spinner;
mod style;
mod terminal;

use options::MarkdownMode;
pub(crate) use options::{RenderOptions, Verbosity};
pub(crate) use renderer::Renderer;

pub(crate) fn markdown_to_string(message: &str) -> String {
    style::render_lines(
        markdown::markdown_lines(
            message,
            &RenderOptions {
                markdown: MarkdownMode::Basic,
                ..RenderOptions::default()
            },
        ),
        &RenderOptions::default(),
    )
}

#[cfg(test)]
mod tests;
