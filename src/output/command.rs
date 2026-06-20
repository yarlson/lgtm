use std::io::{self, Write};

use anyhow::{Context, Result};

use crate::{
    app_server::{TokenUsage, TurnStreamEvent},
    cli::StreamMode,
    output::{
        RenderOptions, Renderer,
        banner::{self, Banner},
    },
};

pub(crate) struct CommandOutput<W> {
    stream_mode: StreamMode,
    renderer: Renderer,
    output: W,
}

impl CommandOutput<io::Stdout> {
    pub(crate) fn stdout(stream_mode: StreamMode) -> Self {
        Self::new(stream_mode, io::stdout())
    }
}

impl<W: Write> CommandOutput<W> {
    pub(crate) fn new(stream_mode: StreamMode, output: W) -> Self {
        Self {
            stream_mode,
            renderer: Renderer::new(RenderOptions::default()),
            output,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_renderer(stream_mode: StreamMode, renderer: Renderer, output: W) -> Self {
        Self {
            stream_mode,
            renderer,
            output,
        }
    }

    #[cfg(test)]
    pub(crate) fn into_inner(self) -> W {
        self.output
    }

    pub(crate) fn with_status_line<T>(
        &mut self,
        label: impl Into<String>,
        action: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.start_status_line(label)?;
        let result = action(self);
        let finish_result = self.finish();
        if result.is_ok() {
            finish_result?;
        }
        result
    }

    pub(crate) fn banner(&mut self, banner: Banner<'_>) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        self.write(banner::render(banner, &RenderOptions::default()))
    }

    pub(crate) fn phase_header(&mut self, phase: u32, title: &str, label: &str) -> Result<()> {
        let rendered = self.renderer.phase_header(phase, title, label);
        self.write(rendered)
    }

    pub(crate) fn start_status_line(&mut self, label: impl Into<String>) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        let rendered = self.renderer.start_status_line(label);
        self.write(rendered)
    }

    pub(crate) fn start_visible_status_line(&mut self, label: impl Into<String>) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        let label = label.into();
        let rendered = self.renderer.start_status_line(label.clone());
        if rendered.is_empty() {
            self.write(format!("{label}\n"))
        } else {
            self.write(rendered)
        }
    }

    pub(crate) fn render_event(&mut self, event: &TurnStreamEvent) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        let rendered = self.renderer.render_event(event);
        self.write(rendered)
    }

    pub(crate) fn tick_on_idle(&mut self, event: &TurnStreamEvent) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty || event != &TurnStreamEvent::Idle {
            return Ok(());
        }
        let rendered = self.renderer.tick();
        self.write(rendered)
    }

    pub(crate) fn finish(&mut self) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        let rendered = self.renderer.finish();
        self.write(rendered)
    }

    pub(crate) fn token_summary(&mut self, usage: TokenUsage) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty || usage.is_zero() {
            return Ok(());
        }
        self.write(token_summary_line(usage))
    }

    pub(crate) fn phase_token_summary(&mut self, phase_id: u32, usage: TokenUsage) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty || usage.is_zero() {
            return Ok(());
        }
        self.write(phase_token_summary_line(phase_id, usage))
    }

    fn write(&mut self, rendered: String) -> Result<()> {
        if rendered.is_empty() {
            return Ok(());
        }

        self.output
            .write_all(rendered.as_bytes())
            .context("failed to write command output")?;
        self.output
            .flush()
            .context("failed to flush command output")
    }
}

fn token_summary_line(usage: TokenUsage) -> String {
    token_summary_line_with_label("Tokens", usage)
}

fn phase_token_summary_line(phase_id: u32, usage: TokenUsage) -> String {
    token_summary_line_with_label(&format!("Phase {phase_id} tokens"), usage)
}

fn token_summary_line_with_label(label: &str, usage: TokenUsage) -> String {
    format!(
        "• {label}: input {} (cached {}), output {}, reasoning {}, total {}\n",
        usage.input_tokens,
        usage.cached_input_tokens,
        usage.output_tokens,
        usage.reasoning_tokens,
        usage.total_tokens
    )
}
