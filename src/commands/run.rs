use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use chrono::Local;

use crate::{
    app_server::{AppServerClient, AppServerConfig, TurnControl},
    cli::{RunArgs, StreamMode},
    git,
    output::{RenderOptions, Renderer},
    phase_index::{self, Phase},
    prompt::{self, PhasePass},
    skills,
};

#[derive(Debug, Clone)]
struct RunConfig {
    root: PathBuf,
    plan_path: PathBuf,
    agents_path: PathBuf,
    start_phase: u32,
    end_phase: Option<u32>,
    sleep_seconds: u64,
    codex_bin: String,
    stream_mode: StreamMode,
    log_dir: PathBuf,
    run_stamp: String,
}

impl RunConfig {
    fn from_args(args: RunArgs) -> Result<Self> {
        let root = match args.root {
            Some(root) => absolutize(root)?,
            None => std::env::current_dir().context("failed to read current directory")?,
        };
        let log_dir = args
            .log_dir
            .map(|path| resolve_under_root(&root, path))
            .unwrap_or_else(|| root.join(".codex-log"));
        let run_stamp = args
            .run_stamp
            .unwrap_or_else(|| Local::now().format("%Y%m%d-%H%M%S").to_string());

        Ok(Self {
            root,
            plan_path: args.plan_path,
            agents_path: args.agents_path,
            start_phase: args.start_phase,
            end_phase: args.end_phase,
            sleep_seconds: args.sleep_seconds,
            codex_bin: args.codex_bin,
            stream_mode: args.stream_mode,
            log_dir,
            run_stamp,
        })
    }

    fn plan_abs(&self) -> PathBuf {
        self.root.join(&self.plan_path)
    }

    fn agents_abs(&self) -> PathBuf {
        self.root.join(&self.agents_path)
    }
}

pub fn run(args: RunArgs) -> Result<()> {
    let config = RunConfig::from_args(args)?;

    require_file(&config.plan_abs(), &config.plan_path)?;
    require_file(&config.agents_abs(), &config.agents_path)?;
    skills::preflight(&config.root)?;
    git::ensure_initialized(&config.root)?;
    skills::install(&config.root)?;

    let mut phase_id = config.start_phase;
    loop {
        if config
            .end_phase
            .is_some_and(|end_phase| phase_id > end_phase)
        {
            break;
        }

        let phases = load_phase_index(&config, phase_id)?;
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
            run_phase_pass(&config, &phase, pass)?;
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

fn load_phase_index(config: &RunConfig, phase_id: u32) -> Result<Vec<Phase>> {
    let plan_text = fs::read_to_string(config.plan_abs())
        .with_context(|| format!("failed to read {}", config.plan_path.display()))?;
    let log_name = format!("{}-phase-{phase_id:02}-index.jsonl", config.run_stamp);
    let mut client = connect_logged_client(
        config,
        Some(phase_index::PARSER_MODEL.to_string()),
        &log_name,
    )?;
    let thread_id = client.start_thread()?;

    let first_turn = client.run_turn(
        &thread_id,
        &phase_index::parser_prompt(&config.plan_path, &plan_text),
    )?;
    let first_output = first_turn.transcript.response_text();
    match phase_index::parse_phase_index(&first_output) {
        Ok(phases) => {
            client.stop()?;
            Ok(phases)
        }
        Err(first_error) => {
            let repair_turn =
                client.run_turn(&thread_id, &phase_index::repair_prompt(&first_output))?;
            let repair_output = repair_turn.transcript.response_text();
            let phases = phase_index::parse_phase_index(&repair_output).with_context(|| {
                format!("phase index parser returned invalid JSON after retry: {first_error}")
            })?;
            client.stop()?;
            Ok(phases)
        }
    }
}

fn run_phase_pass(config: &RunConfig, phase: &Phase, pass: PhasePass) -> Result<()> {
    println!("• Phase {:02} {}: {}", phase.id, pass.label(), phase.title);

    let log_name = format!(
        "{}-phase-{phase:02}-{action}.jsonl",
        config.run_stamp,
        phase = phase.id,
        action = pass.action()
    );
    let mut client = connect_logged_client(config, None, &log_name)?;
    let thread_id = client.start_thread()?;
    let mut renderer = Renderer::new(RenderOptions::default());
    client.run_turn_streaming(
        &thread_id,
        &prompt::phase_prompt(&config.plan_path, &config.agents_path, phase, pass),
        |event| {
            if config.stream_mode == StreamMode::Pretty {
                print!("{}", renderer.render_event(&event));
            }
            TurnControl::Continue
        },
    )?;
    client.stop()
}

fn connect_logged_client(
    config: &RunConfig,
    model: Option<String>,
    log_name: &str,
) -> Result<AppServerClient> {
    fs::create_dir_all(&config.log_dir)
        .with_context(|| format!("failed to create {}", config.log_dir.display()))?;
    let log_path = config.log_dir.join(log_name);
    let log =
        Arc::new(Mutex::new(File::create(&log_path).with_context(|| {
            format!("failed to create {}", log_path.display())
        })?));
    let stream_mode = config.stream_mode;
    let should_print_raw = stream_mode == StreamMode::Raw;

    let app_config = AppServerConfig::for_run(&config.codex_bin, &config.root, model);
    let mut client = AppServerClient::connect(app_config)?;
    let log_for_sink = Arc::clone(&log);
    client.log_raw_messages(move |line| {
        log_for_sink
            .lock()
            .expect("log mutex should not be poisoned")
            .write_all(line.as_bytes())
            .context("failed to write app-server log")?;
        if should_print_raw {
            std::io::stdout()
                .write_all(line.as_bytes())
                .context("failed to write raw app-server output")?;
        }
        Ok(())
    });

    Ok(client)
}

fn require_file(path: &Path, display: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        bail!("required file {} was not found", display.display())
    }
}

fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(std::env::current_dir()
        .context("failed to read current directory")?
        .join(path))
}

fn resolve_under_root(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(config.log_dir, temp.path().join("logs"));
    }

    #[test]
    fn absolute_log_dir_is_preserved() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_dir = temp.path().join("outside");
        let mut args = run_args_with_root(temp.path().join("repo"));
        args.log_dir = Some(log_dir.clone());

        let config = RunConfig::from_args(args).expect("config");

        assert_eq!(config.log_dir, log_dir);
    }
}
