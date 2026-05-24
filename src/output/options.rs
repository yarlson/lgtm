#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Charset {
    Unicode,
    Ascii,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MarkdownMode {
    Basic,
    Plain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    pub color_mode: ColorMode,
    pub charset: Charset,
    pub verbosity: Verbosity,
    pub markdown: MarkdownMode,
    pub max_output_lines: usize,
    pub max_output_chars: usize,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            color_mode: ColorMode::Auto,
            charset: Charset::Unicode,
            verbosity: Verbosity::Normal,
            markdown: MarkdownMode::Basic,
            max_output_lines: 4,
            max_output_chars: 2_000,
        }
    }
}

pub(crate) fn color_enabled(mode: ColorMode) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            supports_color::on_cached(supports_color::Stream::Stdout).is_some()
                && std::env::var_os("NO_COLOR").is_none()
        }
    }
}
