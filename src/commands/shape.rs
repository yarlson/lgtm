use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

mod completion;

use crate::{
    app_server::{AppServerClient, CompletedTurn, TurnControl, TurnStreamEvent},
    cli::{ShapeArgs, StreamMode},
    commands::execution::ExecutionConfig,
    commands::runtime::{CommandRuntime, CommandRuntimeConfig, RuntimeAppServerClient},
    git,
    output::{CommandOutput, banner::Banner, banner::BannerMode},
    prompt::{self, ShapePromptContext},
    skills,
};

use completion::{
    parse_final_plan_marker, validate_final_plan_contract, validate_reported_plan_path,
};

const STARTUP_STATUS: &str = "Started 2 Codex sessions; gathering context";
const HANDOFF_CHAR_BUDGET: usize = 4_000;

#[derive(Debug, Clone)]
struct ShapeConfig {
    runtime: CommandRuntime,
    brief: String,
    plan_path: PathBuf,
    stream_mode: StreamMode,
    max_rounds: u32,
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
            max_rounds,
        } = args;
        if max_rounds == 0 {
            bail!("lgtm shape --max-rounds must be at least 1")
        }
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
            max_rounds,
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
    let mut output = CommandOutput::stdout(config.stream_mode);
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

    let final_plan_path = orchestration_result?;
    finish_result?;
    output.message_line(format!("Final plan: {}", final_plan_path.display()))?;
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
    output: &mut CommandOutput<impl Write>,
) -> Result<PathBuf> {
    output.start_visible_status_line(STARTUP_STATUS)?;
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

    let mut sparring_prompt = prompt::shape_session_a_initial_prompt(&context);
    for round in 1..=config.max_rounds {
        let sparring_turn = run_shape_streaming_turn(
            &mut sessions.session_a,
            &sessions.thread_a,
            &sparring_prompt,
            output,
        )
        .with_context(|| format!("shape session A round {round} sparring turn failed"))?;

        if let Some(marker) = parse_final_plan_marker(&sparring_turn.transcript.response_text())? {
            return complete_shape_plan(config, &marker);
        }

        if round == config.max_rounds {
            break;
        }

        let question = handoff_excerpt("Session A", &sparring_turn, HANDOFF_CHAR_BUDGET)?;
        let answer_turn = run_shape_hidden_turn(
            &mut sessions.session_b,
            &sessions.thread_b,
            &prompt::shape_session_b_question_prompt(&question),
            output,
        )
        .with_context(|| format!("shape session B round {round} evidence answer failed"))?;
        let answer = validate_or_repair_evidence_answer(
            &mut sessions.session_b,
            &sessions.thread_b,
            round,
            &question,
            &answer_turn,
            output,
        )?;
        sparring_prompt = prompt::shape_session_a_answer_prompt(&question, &answer);
    }

    let final_turn = run_shape_streaming_turn(
        &mut sessions.session_a,
        &sessions.thread_a,
        &prompt::shape_session_a_finalization_prompt(&context, config.max_rounds),
        output,
    )
    .context("shape session A finalization turn failed")?;

    if let Some(marker) = parse_final_plan_marker(&final_turn.transcript.response_text())? {
        return complete_shape_plan(config, &marker);
    }

    bail!(
        "shape session A finalization did not report a final plan; expected PLAN_PATH: <path> after --max-rounds={}",
        config.max_rounds
    )
}

fn complete_shape_plan(config: &ShapeConfig, marker: &Path) -> Result<PathBuf> {
    let resolved_plan_path = config.runtime.resolve_root_path(&config.plan_path);
    validate_reported_plan_path(
        config.runtime.root(),
        &config.plan_path,
        &resolved_plan_path,
        marker,
    )?;
    validate_final_plan_contract(&resolved_plan_path)?;
    Ok(resolved_plan_path)
}

fn validate_or_repair_evidence_answer(
    client: &mut AppServerClient,
    thread_id: &str,
    round: u32,
    question: &str,
    answer_turn: &CompletedTurn,
    output: &mut CommandOutput<impl Write>,
) -> Result<String> {
    let invalid_answer = match parse_evidence_answer(&answer_turn.transcript.response_text()) {
        Ok(answer) => return Ok(answer),
        Err(invalid_answer) => invalid_answer,
    };

    let repair_turn = run_shape_hidden_turn(
        client,
        thread_id,
        &prompt::shape_session_b_answer_repair_prompt(
            question,
            &truncate_handoff_text(&invalid_answer, HANDOFF_CHAR_BUDGET),
        ),
        output,
    )
    .with_context(|| format!("shape session B round {round} evidence answer repair failed"))?;

    parse_evidence_answer(&repair_turn.transcript.response_text()).map_err(|invalid_answer| {
        let invalid_answer = truncate_handoff_text(&invalid_answer, HANDOFF_CHAR_BUDGET);
        anyhow::anyhow!(
            "Session B evidence answer remained invalid after one repair attempt: {invalid_answer:?}"
        )
    })
}

fn parse_evidence_answer(response: &str) -> std::result::Result<String, String> {
    let answer = response;
    if answer.lines().count() != 1 {
        return Err(answer.to_string());
    }

    if matches!(answer, "1" | "2" | "3") || is_numbered_correction(answer) {
        return Ok(answer.to_string());
    }

    Err(answer.to_string())
}

fn is_numbered_correction(answer: &str) -> bool {
    let Some((number, correction)) = answer.split_once(", but ") else {
        return false;
    };

    !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
        && !correction.trim().is_empty()
}

fn handoff_excerpt(role: &str, turn: &CompletedTurn, char_budget: usize) -> Result<String> {
    let response = turn.transcript.response_text();
    let response = response.trim();
    if response.is_empty() {
        bail!("{role} produced empty assistant response for shape handoff")
    }

    Ok(format!(
        "{role} assistant excerpt:\n{}",
        truncate_handoff_text(response, char_budget)
    ))
}

fn truncate_handoff_text(text: &str, char_budget: usize) -> String {
    if text.chars().count() <= char_budget {
        return text.to_string();
    }

    let marker = format!("\n[truncated to {char_budget} chars]");
    let marker_len = marker.chars().count();
    if char_budget <= marker_len {
        return marker.chars().take(char_budget).collect();
    }

    let keep_chars = char_budget - marker_len;
    let mut truncated = text.chars().take(keep_chars).collect::<String>();
    truncated.push_str(&marker);
    truncated
}

fn run_shape_hidden_turn(
    client: &mut AppServerClient,
    thread_id: &str,
    prompt: &str,
    output: &mut CommandOutput<impl Write>,
) -> Result<CompletedTurn> {
    run_shape_turn(client, thread_id, prompt, |event| {
        output.tick_on_idle(event)
    })
}

fn run_shape_streaming_turn(
    client: &mut AppServerClient,
    thread_id: &str,
    prompt: &str,
    output: &mut CommandOutput<impl Write>,
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
    use crate::app_server::{TranscriptItem, TurnTranscript};
    use crate::cli::{ExecutionArgs, ExecutionSandbox, ShapeArgs, StreamMode};
    use serde_json::json;

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
    fn rejects_zero_max_rounds() {
        let mut args = shape_args();
        args.brief = Some("brief".to_string());
        args.max_rounds = 0;

        let error = ShapeConfig::from_args(args).expect_err("zero rounds");

        assert!(
            error
                .to_string()
                .contains("--max-rounds must be at least 1")
        );
    }

    #[test]
    fn raw_mode_suppresses_banner_and_status_line() {
        let mut output = CommandOutput::new(StreamMode::Raw, Vec::new());

        output
            .banner(Banner {
                mode: BannerMode::Shape,
                root: Path::new("/repo"),
                codex_bin: "codex",
                execution: "host YOLO",
            })
            .expect("banner");
        output
            .start_visible_status_line(STARTUP_STATUS)
            .expect("status");

        assert!(output.into_inner().is_empty());
    }

    #[test]
    fn pretty_mode_prints_startup_status_when_stdout_is_not_interactive() {
        let mut output = CommandOutput::new(StreamMode::Pretty, Vec::new());

        output
            .start_visible_status_line(STARTUP_STATUS)
            .expect("status");

        let rendered = String::from_utf8(output.into_inner()).expect("utf8");
        assert_eq!(rendered, "Started 2 Codex sessions; gathering context\n");
    }

    #[test]
    fn handoff_excerpt_formats_role_labeled_response_text() {
        let turn = completed_turn("Keep the Rust CLI small.");

        let excerpt = handoff_excerpt("Session A", &turn, 200).expect("excerpt");

        assert_eq!(
            excerpt,
            "Session A assistant excerpt:\nKeep the Rust CLI small."
        );
    }

    #[test]
    fn handoff_excerpt_rejects_empty_response_text() {
        let turn = completed_turn(" \n\t ");

        let error = handoff_excerpt("Session B", &turn, 200).expect_err("empty response");

        assert!(
            error
                .to_string()
                .contains("Session B produced empty assistant response for shape handoff")
        );
    }

    #[test]
    fn handoff_excerpt_truncates_response_text_by_character_budget() {
        let turn = completed_turn("abcdeЖЗИЙ12345678901234567890tail");

        let excerpt = handoff_excerpt("Session B", &turn, 30).expect("excerpt");
        let body = excerpt
            .strip_prefix("Session B assistant excerpt:\n")
            .expect("role label");

        assert_eq!(body.chars().count(), 30);
        assert!(body.starts_with("abcdeЖ"));
        assert!(body.ends_with("[truncated to 30 chars]"));
        assert!(!body.contains("tail"));
    }

    #[test]
    fn handoff_excerpt_uses_assistant_response_without_activity_output() {
        let items = vec![
            TranscriptItem::from_app_server_item(&json!({
                "type": "commandExecution",
                "id": "cmd",
                "command": "cat secret.txt",
                "status": "completed",
                "exitCode": 0,
                "aggregatedOutput": "SECRET_OUTPUT",
            }))
            .expect("command item"),
            TranscriptItem::from_app_server_item(&json!({
                "type": "fileChange",
                "id": "file",
                "status": "completed",
                "changes": [{"kind": "update", "path": "secret.patch"}],
            }))
            .expect("file change item"),
            TranscriptItem::from_app_server_item(&json!({
                "type": "agentMessage",
                "id": "msg",
                "text": "Use option 2.",
                "status": "completed",
            }))
            .expect("agent message item"),
        ];
        let turn = completed_turn_with_items(items);

        let excerpt = handoff_excerpt("Session A", &turn, 200).expect("excerpt");

        assert_eq!(excerpt, "Session A assistant excerpt:\nUse option 2.");
        assert!(!excerpt.contains("SECRET_OUTPUT"));
        assert!(!excerpt.contains("secret.patch"));
    }

    #[test]
    fn evidence_answer_parser_accepts_required_formats() {
        assert_eq!(parse_evidence_answer("1").expect("bare 1"), "1");
        assert_eq!(parse_evidence_answer("2").expect("bare 2"), "2");
        assert_eq!(parse_evidence_answer("3").expect("bare 3"), "3");
        assert_eq!(
            parse_evidence_answer("4, but split the persistence phase first").expect("correction"),
            "4, but split the persistence phase first"
        );
    }

    #[test]
    fn evidence_answer_parser_rejects_prose_and_malformed_answers() {
        for answer in [
            "",
            "I recommend option 2.",
            "2 because it is simpler",
            "2, because it is simpler",
            "2,but missing space",
            "2, but ",
            "- 2",
            "2\nextra",
            " 2",
            "2 ",
        ] {
            assert!(
                parse_evidence_answer(answer).is_err(),
                "answer should be rejected: {answer:?}"
            );
        }
    }

    fn completed_turn(text: &str) -> CompletedTurn {
        let item = TranscriptItem::from_app_server_item(&json!({
            "type": "agentMessage",
            "id": "msg",
            "text": text,
            "status": "completed",
        }))
        .expect("agent message item");
        completed_turn_with_items(vec![item])
    }

    fn completed_turn_with_items(items: Vec<TranscriptItem>) -> CompletedTurn {
        CompletedTurn {
            turn_id: "turn".to_string(),
            status: "completed".to_string(),
            transcript: TurnTranscript::from_items(Vec::new(), items),
            usage: None,
        }
    }
}
