use std::{fs, io::Write, path::PathBuf, thread, time::Duration};

use anyhow::{Context, Result};

use crate::{
    app_server::{AppServerClient, CompletedTurn, TokenUsage, TurnControl, TurnStreamEvent},
    cli::{RunArgs, StreamMode},
    commands::execution::ExecutionConfig,
    commands::runtime::{CommandRuntime, CommandRuntimeConfig, require_file},
    git,
    output::{CommandOutput, banner::Banner, banner::BannerMode},
    phase_index::{self, Phase},
    prompt::{self, PhasePass},
    skills,
};

const DEFAULT_AGENTS_PATH: &str = "AGENTS.md";
const DEFAULT_START_PHASE: u32 = 1;
const DEFAULT_SLEEP_SECONDS: u64 = 600;
const DEFAULT_STREAM_MODE: StreamMode = StreamMode::Pretty;

#[derive(Debug, Clone)]
pub(super) struct RunConfig {
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
        let runtime = CommandRuntime::new(CommandRuntimeConfig {
            root: args.root,
            log_dir: args.log_dir,
            run_stamp: args.run_stamp,
            execution: ExecutionConfig::from_args(args.codex_bin, args.execution),
        })?;

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

    pub(super) fn from_plan_context(runtime: CommandRuntime, plan_path: PathBuf) -> Self {
        Self {
            runtime,
            plan_path,
            agents_path: DEFAULT_AGENTS_PATH.into(),
            start_phase: DEFAULT_START_PHASE,
            end_phase: None,
            sleep_seconds: DEFAULT_SLEEP_SECONDS,
            stream_mode: DEFAULT_STREAM_MODE,
        }
    }

    fn plan_abs(&self) -> PathBuf {
        self.runtime.resolve_root_path(&self.plan_path)
    }

    fn agents_abs(&self) -> PathBuf {
        self.runtime.resolve_root_path(&self.agents_path)
    }

    #[cfg(test)]
    pub(super) fn runtime(&self) -> &CommandRuntime {
        &self.runtime
    }

    #[cfg(test)]
    pub(super) fn plan_path(&self) -> &std::path::Path {
        &self.plan_path
    }

    #[cfg(test)]
    pub(super) fn agents_path(&self) -> &std::path::Path {
        &self.agents_path
    }

    #[cfg(test)]
    pub(super) fn start_phase(&self) -> u32 {
        self.start_phase
    }

    #[cfg(test)]
    pub(super) fn end_phase(&self) -> Option<u32> {
        self.end_phase
    }

    #[cfg(test)]
    pub(super) fn sleep_seconds(&self) -> u64 {
        self.sleep_seconds
    }

    #[cfg(test)]
    pub(super) fn stream_mode(&self) -> StreamMode {
        self.stream_mode
    }
}

pub fn run(args: RunArgs) -> Result<()> {
    let config = RunConfig::from_args(args)?;
    run_config(config)
}

pub(super) fn run_config(config: RunConfig) -> Result<()> {
    let mut output = CommandOutput::stdout(config.stream_mode);
    let mut usage = TokenUsage::default();
    output.banner(Banner {
        mode: BannerMode::Run,
        root: config.runtime.root(),
        codex_bin: config.runtime.app_server_binary(),
        execution: config.runtime.execution_label(),
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

        let mut phase_usage = TokenUsage::default();
        let phases = output.with_status_line("parsing plan phases", |output| {
            load_phase_index(&config, phase_id, output, &mut phase_usage)
        })?;
        let end_phase = match config.end_phase {
            Some(end_phase) => end_phase,
            None => phase_index::detected_end_phase(&phases, &config.plan_path)?,
        };
        if phase_id > end_phase {
            usage = usage.add(phase_usage);
            break;
        }

        let Some(phase) = phase_index::next_phase(&phases, phase_id, end_phase) else {
            usage = usage.add(phase_usage);
            break;
        };
        run_phase(&config, &phase, &mut output, &mut phase_usage)?;
        usage = usage.add(phase_usage);

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

    output.token_summary(usage)
}

fn load_phase_index(
    config: &RunConfig,
    phase_id: u32,
    output: &mut CommandOutput<impl Write>,
    usage: &mut TokenUsage,
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
    add_usage(usage, &first_turn);
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
            add_usage(usage, &repair_turn);
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
    output: &mut CommandOutput<impl Write>,
) -> Result<CompletedTurn> {
    run_streaming_turn(
        client,
        thread_id,
        prompt,
        |event| output.tick_on_idle(event),
        phase_index::PARSER_REASONING_EFFORT,
    )
}

fn run_phase(
    config: &RunConfig,
    phase: &Phase,
    output: &mut CommandOutput<impl Write>,
    usage: &mut TokenUsage,
) -> Result<()> {
    let first_pass = PhasePass::Implement;
    let log_name = phase_pass_log_name(config, phase, first_pass);
    let mut client = config.runtime.connect_logged_app_server(
        None,
        &log_name,
        config.stream_mode == StreamMode::Raw,
    )?;
    let thread_id = client.start_thread()?;

    run_phase_pass(
        config,
        phase,
        first_pass,
        &mut client,
        &thread_id,
        output,
        usage,
    )?;
    for pass in PhasePass::ALL.into_iter().skip(1) {
        let log_name = phase_pass_log_name(config, phase, pass);
        config.runtime.set_log_sink(
            &mut client,
            &log_name,
            config.stream_mode == StreamMode::Raw,
        )?;
        run_phase_pass(config, phase, pass, &mut client, &thread_id, output, usage)?;
    }

    client.stop()?;
    output.phase_token_summary(phase.id, *usage)
}

fn phase_pass_log_name(config: &RunConfig, phase: &Phase, pass: PhasePass) -> String {
    format!(
        "{}-phase-{phase:02}-{action}.jsonl",
        config.runtime.run_stamp(),
        phase = phase.id,
        action = pass.action()
    )
}

fn run_phase_pass(
    config: &RunConfig,
    phase: &Phase,
    pass: PhasePass,
    client: &mut AppServerClient,
    thread_id: &str,
    output: &mut CommandOutput<impl Write>,
    usage: &mut TokenUsage,
) -> Result<()> {
    output.phase_header(phase.id, &phase.title, pass.label())?;
    let turn = run_streaming_turn(
        client,
        thread_id,
        &prompt::phase_prompt(&config.plan_path, &config.agents_path, phase, pass),
        |event| output.render_event(event),
        pass.reasoning_effort(),
    )?;
    add_usage(usage, &turn);
    output.finish()
}

fn run_streaming_turn(
    client: &mut AppServerClient,
    thread_id: &str,
    prompt: &str,
    mut render_event: impl FnMut(&TurnStreamEvent) -> Result<()>,
    effort: &str,
) -> Result<CompletedTurn> {
    let mut output_result = Ok(());
    let turn = client.run_turn_streaming_with_effort(thread_id, prompt, effort, |event| {
        if output_result.is_ok() {
            output_result = render_event(&event);
        }
        TurnControl::Continue
    })?;
    output_result?;
    Ok(turn)
}

fn add_usage(total: &mut TokenUsage, turn: &CompletedTurn) {
    if let Some(usage) = turn.usage {
        *total = total.add(usage);
    }
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
            execution: crate::cli::ExecutionArgs::default(),
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
    fn raw_mode_suppresses_banner() {
        let mut output = CommandOutput::new(StreamMode::Raw, FlushCountingWriter::default());
        let root = PathBuf::from("/repo");

        output
            .banner(Banner {
                mode: BannerMode::Run,
                root: root.as_path(),
                codex_bin: "codex",
                execution: "host YOLO",
            })
            .expect("banner");

        let output = output.into_inner();
        assert!(output.content.is_empty());
        assert_eq!(output.flushes, 0);
    }

    #[test]
    fn pretty_mode_prints_token_summary() {
        let mut output = CommandOutput::new(StreamMode::Pretty, FlushCountingWriter::default());

        output
            .token_summary(TokenUsage {
                input_tokens: 100,
                cached_input_tokens: 80,
                output_tokens: 20,
                reasoning_tokens: 5,
                total_tokens: 120,
            })
            .expect("summary");

        let output = output.into_inner();
        let rendered = String::from_utf8(output.content).expect("utf8");
        assert_eq!(
            rendered,
            "• Tokens: input 100 (cached 80), output 20, reasoning 5, total 120\n"
        );
    }

    #[test]
    fn pretty_mode_prints_phase_token_summary() {
        let mut output = CommandOutput::new(StreamMode::Pretty, FlushCountingWriter::default());

        output
            .phase_token_summary(
                2,
                TokenUsage {
                    input_tokens: 100,
                    cached_input_tokens: 80,
                    output_tokens: 20,
                    reasoning_tokens: 5,
                    total_tokens: 120,
                },
            )
            .expect("summary");

        let output = output.into_inner();
        let rendered = String::from_utf8(output.content).expect("utf8");
        assert_eq!(
            rendered,
            "• Phase 2 tokens: input 100 (cached 80), output 20, reasoning 5, total 120\n"
        );
    }

    #[test]
    fn raw_mode_suppresses_token_summary() {
        let mut output = CommandOutput::new(StreamMode::Raw, FlushCountingWriter::default());

        output
            .token_summary(TokenUsage {
                input_tokens: 100,
                cached_input_tokens: 80,
                output_tokens: 20,
                reasoning_tokens: 5,
                total_tokens: 120,
            })
            .expect("summary");

        assert!(output.into_inner().content.is_empty());
    }
}
