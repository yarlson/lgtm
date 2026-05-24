mod item;
mod markdown;
mod options;
mod renderer;
pub(crate) mod spinner;
mod style;
mod terminal;

pub(crate) use options::{Charset, ColorMode, MarkdownMode, RenderOptions, Verbosity};
pub(crate) use renderer::Renderer;

pub(crate) fn markdown_to_string(message: &str) -> String {
    style::render_lines(
        markdown::markdown_lines(message, MarkdownMode::Basic),
        &RenderOptions::default(),
    )
}

#[cfg(test)]
mod tests;
