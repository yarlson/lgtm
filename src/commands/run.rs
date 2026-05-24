use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    thread,
    time::Duration,
};

use anyhow::{Context, Result};

use crate::{
    app_server::{AppServerClient, CompletedTurn, TurnControl, TurnStreamEvent},
    cli::{RunArgs, StreamMode},
    commands::runtime::{CommandRuntime, require_file},
    git,
    output::{
        RenderOptions, Renderer,
        banner::{self, Banner, BannerMode},
    },
    phase_index::{self, Phase},
    prompt::{self, PhasePass},
    skills,
};

#[derive(Debug, Clone)]
struct RunConfig {
    runtime: CommandRuntime,
    plan_path: PathBuf,
    agents_path: PathBuf,
    start_phase: u32,
    end_phase: Option<u32>,
    sleep_seconds: u64,
    stream_mode: StreamMode,
}

impl RunConfig {
    fn from_args(args: RunArgs) -> Result<Self> {
        let runtime = CommandRuntime::new(args.root, args.codex_bin, args.log_dir, args.run_stamp)?;

        Ok(Self {
            runtime,
            plan_path: args.plan_path,
            agents_path: args.agents_path,
            start_phase: args.start_phase,
            end_phase: args.end_phase,
            sleep_seconds: args.sleep_seconds,
            stream_mode: args.stream_mode,
        })
    }

    fn plan_abs(&self) -> PathBuf {
        self.runtime.resolve_root_path(&self.plan_path)
    }

    fn agents_abs(&self) -> PathBuf {
        self.runtime.resolve_root_path(&self.agents_path)
    }
}

pub fn run(args: RunArgs) -> Result<()> {
    let config = RunConfig::from_args(args)?;
    let mut output = RunOutput::stdout(config.stream_mode);
    output.banner(Banner {
        mode: BannerMode::Run,
        root: config.runtime.root(),
        codex_bin: config.runtime.codex_bin(),
    })?;

    require_file(&config.plan_abs(), &config.plan_path)?;
    require_file(&config.agents_abs(), &config.agents_path)?;
    skills::preflight(config.runtime.root())?;
    git::ensure_initialized(config.runtime.root())?;
    skills::install(config.runtime.root())?;

    let mut phase_id = config.start_phase;
    loop {
        if config
            .end_phase
            .is_some_and(|end_phase| phase_id > end_phase)
        {
            break;
        }

        let phases = output.with_status_line("parsing plan phases", |output| {
            load_phase_index(&config, phase_id, output)
        })?;
        let end_phase = match config.end_phase {
            Some(end_phase) => end_phase,
            None => phase_index::detected_end_phase(&phases, &config.plan_path)?,
        };
        if phase_id > end_phase {
            break;
        }

        let Some(phase) = phase_index::next_phase(&phases, phase_id, end_phase) else {
            break;
        };
        for pass in PhasePass::ALL {
            run_phase_pass(&config, &phase, pass, &mut output)?;
        }

        if phase.id < end_phase {
            println!(
                "• Waiting {}s before Phase {}",
                config.sleep_seconds,
                phase.id + 1
            );
            thread::sleep(Duration::from_secs(config.sleep_seconds));
        }

        phase_id = phase.id + 1;
    }

    Ok(())
}

struct RunOutput<W> {
    stream_mode: StreamMode,
    renderer: Renderer,
    output: W,
}

impl RunOutput<io::Stdout> {
    fn stdout(stream_mode: StreamMode) -> Self {
        Self::new(stream_mode, io::stdout())
    }
}

impl<W: Write> RunOutput<W> {
    fn new(stream_mode: StreamMode, output: W) -> Self {
        Self {
            stream_mode,
            renderer: Renderer::new(RenderOptions::default()),
            output,
        }
    }

    fn with_status_line<T>(
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

    fn start_status_line(&mut self, label: impl Into<String>) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        let rendered = self.renderer.start_status_line(label);
        self.write(rendered)
    }

    fn banner(&mut self, banner: Banner<'_>) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        self.write(banner::render(banner, &RenderOptions::default()))
    }

    fn phase_header(&mut self, phase: &Phase, pass: PhasePass) -> Result<()> {
        let rendered = self
            .renderer
            .phase_header(phase.id, &phase.title, pass.label());
        self.write(rendered)
    }

    fn render_event(&mut self, event: &TurnStreamEvent) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        let rendered = self.renderer.render_event(event);
        self.write(rendered)
    }

    fn tick_on_idle(&mut self, event: &TurnStreamEvent) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty || event != &TurnStreamEvent::Idle {
            return Ok(());
        }
        let rendered = self.renderer.tick();
        self.write(rendered)
    }

    fn finish(&mut self) -> Result<()> {
        let rendered = self.renderer.finish();
        self.write(rendered)
    }

    fn write(&mut self, rendered: String) -> Result<()> {
        write_run_output(&mut self.output, rendered)
    }
}

fn load_phase_index(
    config: &RunConfig,
    phase_id: u32,
    output: &mut RunOutput<impl Write>,
) -> Result<Vec<Phase>> {
    let plan_text = fs::read_to_string(config.plan_abs())
        .with_context(|| format!("failed to read {}", config.plan_path.display()))?;
    let log_name = format!(
        "{}-phase-{phase_id:02}-index.jsonl",
        config.runtime.run_stamp()
    );
    let mut client = config.runtime.connect_logged_app_server(
        Some(phase_index::PARSER_MODEL.to_string()),
        &log_name,
        config.stream_mode == StreamMode::Raw,
    )?;
    let thread_id = client.start_thread()?;

    let first_turn = run_phase_index_turn(
        &mut client,
        &thread_id,
        &phase_index::parser_prompt(&config.plan_path, &plan_text),
        output,
    )?;
    let first_output = first_turn.transcript.response_text();
    match phase_index::parse_phase_index(&first_output) {
        Ok(phases) => {
            client.stop()?;
            Ok(phases)
        }
        Err(first_error) => {
            let repair_turn = run_phase_index_turn(
                &mut client,
                &thread_id,
                &phase_index::repair_prompt(&first_output),
                output,
            )?;
            let repair_output = repair_turn.transcript.response_text();
            let phases = phase_index::parse_phase_index(&repair_output).with_context(|| {
                format!("phase index parser returned invalid JSON after retry: {first_error}")
            })?;
            client.stop()?;
            Ok(phases)
        }
    }
}

fn run_phase_index_turn(
    client: &mut AppServerClient,
    thread_id: &str,
    prompt: &str,
    output: &mut RunOutput<impl Write>,
) -> Result<CompletedTurn> {
    run_streaming_turn(client, thread_id, prompt, |event| {
        output.tick_on_idle(event)
    })
}

fn run_phase_pass(
    config: &RunConfig,
    phase: &Phase,
    pass: PhasePass,
    output: &mut RunOutput<impl Write>,
) -> Result<()> {
    let log_name = format!(
        "{}-phase-{phase:02}-{action}.jsonl",
        config.runtime.run_stamp(),
        phase = phase.id,
        action = pass.action()
    );
    let mut client = config.runtime.connect_logged_app_server(
        None,
        &log_name,
        config.stream_mode == StreamMode::Raw,
    )?;
    let thread_id = client.start_thread()?;
    output.phase_header(phase, pass)?;
    run_streaming_turn(
        &mut client,
        &thread_id,
        &prompt::phase_prompt(&config.plan_path, &config.agents_path, phase, pass),
        |event| output.render_event(event),
    )?;
    output.finish()?;
    client.stop()
}

fn run_streaming_turn(
    client: &mut AppServerClient,
    thread_id: &str,
    prompt: &str,
    mut render_event: impl FnMut(&TurnStreamEvent) -> Result<()>,
) -> Result<CompletedTurn> {
    let mut output_result = Ok(());
    let turn = client.run_turn_streaming(thread_id, prompt, |event| {
        if output_result.is_ok() {
            output_result = render_event(&event);
        }
        TurnControl::Continue
    })?;
    output_result?;
    Ok(turn)
}

fn write_run_output(output: &mut impl Write, rendered: String) -> Result<()> {
    if rendered.is_empty() {
        return Ok(());
    }

    output
        .write_all(rendered.as_bytes())
        .context("failed to write run output")?;
    output.flush().context("failed to flush run output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FlushCountingWriter {
        content: Vec<u8>,
        flushes: usize,
    }

    impl Write for FlushCountingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.content.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    fn run_args_with_root(root: PathBuf) -> RunArgs {
        RunArgs {
            root: Some(root),
            plan_path: "PLAN.md".into(),
            agents_path: "AGENTS.md".into(),
            start_phase: 1,
            end_phase: Some(1),
            sleep_seconds: 0,
            codex_bin: "codex".to_string(),
            stream_mode: StreamMode::Pretty,
            log_dir: None,
            run_stamp: Some("test".to_string()),
        }
    }

    #[test]
    fn relative_log_dir_is_resolved_under_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut args = run_args_with_root(temp.path().to_path_buf());
        args.log_dir = Some("logs".into());

        let config = RunConfig::from_args(args).expect("config");

        assert_eq!(config.runtime.log_dir(), temp.path().join("logs"));
    }

    #[test]
    fn absolute_log_dir_is_preserved() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_dir = temp.path().join("outside");
        let mut args = run_args_with_root(temp.path().join("repo"));
        args.log_dir = Some(log_dir.clone());

        let config = RunConfig::from_args(args).expect("config");

        assert_eq!(config.runtime.log_dir(), log_dir);
    }

    #[test]
    fn run_output_is_flushed_after_each_render() {
        let mut output = FlushCountingWriter::default();

        write_run_output(&mut output, "status line".to_string()).expect("write output");

        assert_eq!(output.content, b"status line");
        assert_eq!(output.flushes, 1);
    }

    #[test]
    fn raw_mode_suppresses_banner() {
        let mut output = RunOutput::new(StreamMode::Raw, FlushCountingWriter::default());
        let root = PathBuf::from("/repo");

        output
            .banner(Banner {
                mode: BannerMode::Run,
                root: root.as_path(),
                codex_bin: "codex",
            })
            .expect("banner");

        assert!(output.output.content.is_empty());
        assert_eq!(output.output.flushes, 0);
    }
}
