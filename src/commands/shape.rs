use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{
    app_server::{AppServerClient, CompletedTurn, TurnControl, TurnStreamEvent},
    cli::{ShapeArgs, StreamMode},
    commands::execution::ExecutionConfig,
    commands::runtime::{CommandRuntime, CommandRuntimeConfig, RuntimeAppServerClient},
    git,
    output::{
        RenderOptions, Renderer,
        banner::{self, Banner, BannerMode},
    },
    prompt::{self, ShapePromptContext},
    skills,
};

const STARTUP_STATUS: &str = "Started 2 Codex sessions; gathering context";

#[derive(Debug, Clone)]
struct ShapeConfig {
    runtime: CommandRuntime,
    brief: String,
    plan_path: PathBuf,
    stream_mode: StreamMode,
}

impl ShapeConfig {
    fn from_args(args: ShapeArgs) -> Result<Self> {
        let ShapeArgs {
            brief,
            brief_file,
            root,
            plan_path,
            codex_bin,
            execution,
            stream_mode,
            log_dir,
            run_stamp,
            max_rounds: _,
        } = args;
        let runtime = CommandRuntime::new(CommandRuntimeConfig {
            root,
            log_dir,
            run_stamp,
            execution: ExecutionConfig::from_args(codex_bin, execution),
        })?;
        let brief = read_brief_source(brief, brief_file.as_deref(), runtime.root())?;

        Ok(Self {
            runtime,
            brief,
            plan_path,
            stream_mode,
        })
    }

    #[cfg(test)]
    fn runtime(&self) -> &CommandRuntime {
        &self.runtime
    }
}

pub fn run(args: ShapeArgs) -> Result<()> {
    let config = ShapeConfig::from_args(args)?;
    run_config(config)
}

fn run_config(config: ShapeConfig) -> Result<()> {
    let mut output = ShapeOutput::stdout(config.stream_mode);
    output.banner(Banner {
        mode: BannerMode::Shape,
        root: config.runtime.root(),
        codex_bin: config.runtime.app_server_binary(),
        execution: config.runtime.execution_label(),
    })?;

    skills::preflight(config.runtime.root())?;
    git::ensure_initialized(config.runtime.root())?;
    skills::install(config.runtime.root())?;

    let mut sessions = start_shape_sessions(&config)?;
    let orchestration_result = run_shape_orchestration(&config, &mut sessions, &mut output);
    let finish_result = output.finish();
    let stop_result = stop_shape_sessions(sessions);

    orchestration_result?;
    finish_result?;
    stop_result?;
    Ok(())
}

struct ShapeSessions {
    session_a: RuntimeAppServerClient,
    thread_a: String,
    session_b: RuntimeAppServerClient,
    thread_b: String,
}

fn start_shape_sessions(config: &ShapeConfig) -> Result<ShapeSessions> {
    let mut session_a =
        connect_shape_session(config, "a").context("failed to start shape session A")?;
    let thread_a = match session_a
        .start_thread()
        .context("failed to start shape session A thread")
    {
        Ok(thread_a) => thread_a,
        Err(error) => {
            let _ = session_a.stop();
            return Err(error);
        }
    };

    let mut session_b =
        match connect_shape_session(config, "b").context("failed to start shape session B") {
            Ok(session_b) => session_b,
            Err(error) => {
                let _ = session_a.stop();
                return Err(error);
            }
        };
    let thread_b = match session_b
        .start_thread()
        .context("failed to start shape session B thread")
    {
        Ok(thread_b) => thread_b,
        Err(error) => {
            let _ = session_b.stop();
            let _ = session_a.stop();
            return Err(error);
        }
    };

    Ok(ShapeSessions {
        session_a,
        thread_a,
        session_b,
        thread_b,
    })
}

fn stop_shape_sessions(sessions: ShapeSessions) -> Result<()> {
    let stop_a = sessions
        .session_a
        .stop()
        .context("failed to stop shape session A");
    let stop_b = sessions
        .session_b
        .stop()
        .context("failed to stop shape session B");
    stop_a?;
    stop_b
}

fn connect_shape_session(config: &ShapeConfig, role: &str) -> Result<RuntimeAppServerClient> {
    let log_name = format!("{}-shape-{role}-001.jsonl", config.runtime.run_stamp());
    config
        .runtime
        .connect_logged_app_server(None, &log_name, config.stream_mode == StreamMode::Raw)
}

fn run_shape_orchestration(
    config: &ShapeConfig,
    sessions: &mut ShapeSessions,
    output: &mut ShapeOutput<impl Write>,
) -> Result<()> {
    output.start_status_line(STARTUP_STATUS)?;
    let context = ShapePromptContext {
        brief: &config.brief,
        root: config.runtime.root(),
        plan_path: &config.runtime.resolve_root_path(&config.plan_path),
    };

    run_shape_hidden_turn(
        &mut sessions.session_b,
        &sessions.thread_b,
        &prompt::shape_session_b_initial_prompt(&context),
        output,
    )
    .context("shape session B round 0 initial discovery failed")?;

    run_shape_streaming_turn(
        &mut sessions.session_a,
        &sessions.thread_a,
        &prompt::shape_session_a_initial_prompt(&context),
        output,
    )
    .context("shape session A round 1 sparring turn failed")?;

    bail!("lgtm shape loop completion is not implemented yet")
}

fn run_shape_hidden_turn(
    client: &mut AppServerClient,
    thread_id: &str,
    prompt: &str,
    output: &mut ShapeOutput<impl Write>,
) -> Result<CompletedTurn> {
    run_shape_turn(client, thread_id, prompt, |event| {
        output.tick_on_idle(event)
    })
}

fn run_shape_streaming_turn(
    client: &mut AppServerClient,
    thread_id: &str,
    prompt: &str,
    output: &mut ShapeOutput<impl Write>,
) -> Result<CompletedTurn> {
    run_shape_turn(client, thread_id, prompt, |event| {
        output.render_event(event)
    })
}

fn run_shape_turn(
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

struct ShapeOutput<W> {
    stream_mode: StreamMode,
    renderer: Renderer,
    output: W,
}

impl ShapeOutput<io::Stdout> {
    fn stdout(stream_mode: StreamMode) -> Self {
        Self::new(stream_mode, io::stdout())
    }
}

impl<W: Write> ShapeOutput<W> {
    fn new(stream_mode: StreamMode, output: W) -> Self {
        Self {
            stream_mode,
            renderer: Renderer::new(RenderOptions::default()),
            output,
        }
    }

    fn banner(&mut self, banner: Banner<'_>) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        self.write(banner::render(banner, &RenderOptions::default()))
    }

    fn start_status_line(&mut self, label: impl Into<String>) -> Result<()> {
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        let label = label.into();
        let rendered = self.renderer.start_status_line(label.clone());
        let rendered = if rendered.is_empty() {
            format!("{label}\n")
        } else {
            rendered
        };
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
        if self.stream_mode != StreamMode::Pretty {
            return Ok(());
        }
        let rendered = self.renderer.finish();
        self.write(rendered)
    }

    fn write(&mut self, rendered: String) -> Result<()> {
        if rendered.is_empty() {
            return Ok(());
        }
        self.output
            .write_all(rendered.as_bytes())
            .context("failed to write shape output")?;
        self.output.flush().context("failed to flush shape output")
    }
}

fn read_brief_source(
    brief: Option<String>,
    brief_file: Option<&Path>,
    root: &Path,
) -> Result<String> {
    let content = match (brief, brief_file) {
        (Some(_), Some(_)) => {
            bail!(
                "lgtm shape accepts exactly one brief source; pass either BRIEF or --brief-file PATH, not both"
            )
        }
        (None, None) => {
            bail!("lgtm shape requires a brief source; pass BRIEF or --brief-file PATH")
        }
        (Some(brief), None) => brief,
        (None, Some(brief_file)) => {
            let path = if brief_file.is_absolute() {
                brief_file.to_path_buf()
            } else {
                root.join(brief_file)
            };
            fs::read_to_string(&path)
                .with_context(|| format!("failed to read brief file {}", path.display()))?
        }
    };

    let brief = content.trim();
    if brief.is_empty() {
        bail!("lgtm shape brief cannot be empty; provide non-whitespace content")
    }

    Ok(brief.to_string())
}

#[cfg(test)]
fn read_brief(args: &ShapeArgs) -> Result<String> {
    read_brief_source(
        args.brief.clone(),
        args.brief_file.as_deref(),
        &target_root(args.root.as_deref())?,
    )
}

#[cfg(test)]
fn target_root(root: Option<&Path>) -> Result<std::path::PathBuf> {
    match root {
        Some(path) if path.is_absolute() => Ok(path.to_path_buf()),
        Some(path) => Ok(std::env::current_dir()
            .context("failed to read current directory")?
            .join(path)),
        None => std::env::current_dir().context("failed to read current directory"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{ExecutionArgs, ExecutionSandbox, ShapeArgs, StreamMode};

    fn shape_args() -> ShapeArgs {
        ShapeArgs {
            brief: None,
            brief_file: None,
            root: None,
            plan_path: "PLAN.md".into(),
            codex_bin: "codex".to_string(),
            execution: ExecutionArgs::default(),
            stream_mode: StreamMode::Pretty,
            log_dir: None,
            run_stamp: None,
            max_rounds: 12,
        }
    }

    #[test]
    fn accepts_string_brief() {
        let mut args = shape_args();
        args.brief = Some("  ship smaller phases  ".to_string());

        assert_eq!(read_brief(&args).expect("brief"), "ship smaller phases");
    }

    #[test]
    fn accepts_file_brief_relative_to_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        fs::create_dir(root.join("docs")).expect("docs");
        fs::write(root.join("docs/brief.md"), "\nshape this\n").expect("brief");
        let mut args = shape_args();
        args.root = Some(root);
        args.brief_file = Some("docs/brief.md".into());

        assert_eq!(read_brief(&args).expect("brief"), "shape this");
    }

    #[test]
    fn accepts_absolute_file_brief() {
        let temp = tempfile::tempdir().expect("tempdir");
        let brief_file = temp.path().join("brief.md");
        fs::write(&brief_file, "shape absolute").expect("brief");
        let mut args = shape_args();
        args.root = Some(temp.path().join("other-root"));
        args.brief_file = Some(brief_file);

        assert_eq!(read_brief(&args).expect("brief"), "shape absolute");
    }

    #[test]
    fn rejects_missing_brief_source() {
        let error = read_brief(&shape_args()).expect_err("missing source");

        assert!(
            error
                .to_string()
                .contains("requires a brief source; pass BRIEF or --brief-file PATH")
        );
    }

    #[test]
    fn rejects_both_brief_sources() {
        let mut args = shape_args();
        args.brief = Some("brief".to_string());
        args.brief_file = Some("docs/brief.md".into());

        let error = read_brief(&args).expect_err("both sources");

        assert!(
            error
                .to_string()
                .contains("pass either BRIEF or --brief-file PATH, not both")
        );
    }

    #[test]
    fn rejects_missing_file_brief() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut args = shape_args();
        args.root = Some(temp.path().to_path_buf());
        args.brief_file = Some("missing.md".into());

        let error = read_brief(&args).expect_err("missing file");

        assert!(error.to_string().contains("failed to read brief file"));
        assert!(error.to_string().contains("missing.md"));
    }

    #[test]
    fn rejects_empty_brief() {
        let mut args = shape_args();
        args.brief = Some(" \n\t ".to_string());

        let error = read_brief(&args).expect_err("empty brief");

        assert!(
            error
                .to_string()
                .contains("brief cannot be empty; provide non-whitespace content")
        );
    }

    #[test]
    fn rejects_empty_file_brief() {
        let temp = tempfile::tempdir().expect("tempdir");
        let brief_file = temp.path().join("brief.md");
        fs::write(&brief_file, " \n\t ").expect("brief");
        let mut args = shape_args();
        args.brief_file = Some(brief_file);

        let error = read_brief(&args).expect_err("empty brief");

        assert!(
            error
                .to_string()
                .contains("brief cannot be empty; provide non-whitespace content")
        );
    }

    #[test]
    fn relative_log_dir_is_resolved_under_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut args = shape_args();
        args.root = Some(temp.path().to_path_buf());
        args.brief = Some("brief".to_string());
        args.log_dir = Some("logs".into());

        let config = ShapeConfig::from_args(args).expect("config");

        assert_eq!(config.runtime().log_dir(), temp.path().join("logs"));
    }

    #[test]
    fn absolute_log_dir_is_preserved() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_dir = temp.path().join("outside");
        let mut args = shape_args();
        args.root = Some(temp.path().join("repo"));
        args.brief = Some("brief".to_string());
        args.log_dir = Some(log_dir.clone());

        let config = ShapeConfig::from_args(args).expect("config");

        assert_eq!(config.runtime().log_dir(), log_dir);
    }

    #[test]
    fn preserves_apple_container_execution_context() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");
        let mut args = shape_args();
        args.root = Some(root);
        args.brief = Some("brief".to_string());
        args.execution.sandbox = ExecutionSandbox::AppleContainer;
        args.execution.sandbox_image = "example.com/lgtm-codex:test".to_string();
        args.execution.container_bin = "container-test".to_string();
        args.execution.codex_auth_path = Some(auth_path.clone());

        let config = ShapeConfig::from_args(args).expect("config");

        assert_eq!(config.runtime().execution_label(), "Apple Container");
        assert_eq!(config.runtime().app_server_binary(), "codex");
        assert_eq!(
            config
                .runtime()
                .apple_container_execution_details()
                .expect("apple container details"),
            (
                "container-test",
                "example.com/lgtm-codex:test",
                auth_path.as_path()
            )
        );
    }

    #[test]
    fn raw_mode_suppresses_banner_and_status_line() {
        let mut output = ShapeOutput::new(StreamMode::Raw, Vec::new());

        output
            .banner(Banner {
                mode: BannerMode::Shape,
                root: Path::new("/repo"),
                codex_bin: "codex",
                execution: "host YOLO",
            })
            .expect("banner");
        output.start_status_line(STARTUP_STATUS).expect("status");

        assert!(output.output.is_empty());
    }

    #[test]
    fn pretty_mode_prints_startup_status_when_stdout_is_not_interactive() {
        let mut output = ShapeOutput::new(StreamMode::Pretty, Vec::new());

        output.start_status_line(STARTUP_STATUS).expect("status");

        let rendered = String::from_utf8(output.output).expect("utf8");
        assert_eq!(rendered, "Started 2 Codex sessions; gathering context\n");
    }
}
