use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::IsTerminal;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::SystemTime;

use crate::Error;
use crate::cli::Config;
use crate::cli::PlanConfig;
use crate::cli::StreamMode;
use crate::composer;
use crate::composer::ComposerSubmission;
use crate::events::CodexEvent;
use crate::events::EventPayload;
use crate::events::ItemPayload;
use crate::git;
use crate::plan;
use crate::prompt;
use crate::render::Renderer;
use crate::render::Spinner;
use crate::render::random_spinner_text;
use crate::skills;

const INITIAL_PLAN_SPINNER_TEXT: &str = "exploring directory";

pub fn run_plan(config: Config) -> Result<(), Error> {
    let mut renderer = Renderer::new();

    plan::require_file(&config.plan_abs(), &config.plan_path)?;
    plan::require_file(&config.agents_abs(), &config.agents_path)?;
    skills::preflight(&config.root)?;
    git::ensure_initialized(&config.root)?;
    skills::install(&config.root)?;

    let mut phase_number = config.start_phase;
    loop {
        let plan_text = plan::load(&config.plan_abs())?;
        let end_phase = match config.end_phase {
            Some(end_phase) => end_phase,
            None => plan::detect_end_phase(&plan_text).ok_or_else(|| {
                Error::message(format!(
                    "could not detect end phase from {}",
                    config.plan_path.display()
                ))
            })?,
        };
        if phase_number > end_phase {
            break;
        }

        let phase = plan::phase(&plan_text, phase_number).ok_or_else(|| {
            Error::message(format!(
                "Phase {phase_number} was not found in {}",
                config.plan_path.display()
            ))
        })?;

        for pass in prompt::PhasePass::ALL {
            run_phase_prompt(
                &config,
                &mut renderer,
                &phase,
                pass.action(),
                prompt::phase_prompt(&config.plan_path, &config.agents_path, &phase, pass),
            )?;
        }

        if phase.number < end_phase {
            renderer.sleep(config.sleep_seconds, phase.number + 1);
            thread::sleep(Duration::from_secs(config.sleep_seconds));
        }

        phase_number += 1;
    }

    Ok(())
}

pub fn run_planning(config: PlanConfig) -> Result<(), Error> {
    require_planning_tty()?;
    skills::preflight(&config.root)?;
    git::ensure_initialized(&config.root)?;
    skills::install(&config.root)?;

    let plan_before = PlanSnapshot::capture(&config.plan_abs())?;
    let first_prompt = prompt::plan_initial_prompt(&config.plan_path, config.brief.as_deref());
    let mut turn = run_planning_turn(
        &config,
        PlanTurn::First,
        first_prompt,
        1,
        &plan_before,
        INITIAL_PLAN_SPINNER_TEXT,
    )?;
    let thread_id = turn
        .thread_id
        .take()
        .expect("first planning turn requires thread id");
    let mut turn_number = 1;

    loop {
        if let Some(message) = turn.last_agent_message.take() {
            print_planning_message(&message);
        }

        if turn.plan_changed {
            return Ok(());
        }

        let resume_prompt = match composer::read_inline_answer()? {
            ComposerSubmission::Quit => return Ok(()),
            ComposerSubmission::Finish => prompt::plan_resume_prompt("/finish"),
            ComposerSubmission::Answer(answer) => prompt::plan_resume_prompt(&answer),
        };

        turn_number += 1;
        turn = run_planning_turn(
            &config,
            PlanTurn::Resume {
                thread_id: &thread_id,
            },
            resume_prompt,
            turn_number,
            &plan_before,
            random_spinner_text(),
        )?;
    }
}

fn print_planning_message(message: &str) {
    let rendered = crate::render::plan_message_to_string(message);
    if !rendered.is_empty() {
        println!("{rendered}");
    }
}

fn require_planning_tty() -> Result<(), Error> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        Ok(())
    } else {
        Err(Error::message(
            "lgtm plan requires interactive stdin and stdout; run it from a TTY",
        ))
    }
}

fn run_phase_prompt(
    config: &Config,
    renderer: &mut Renderer,
    phase: &plan::Phase,
    action: &str,
    prompt: String,
) -> Result<(), Error> {
    let log_name = format!(
        "{}-phase-{phase:02}-{action}.jsonl",
        config.run_stamp,
        phase = phase.number
    );
    let log_path = config.log_dir.join(log_name);

    renderer.phase_header(phase.number, &phase.title, action);

    fs::create_dir_all(&config.log_dir).map_err(|source| Error::io(&config.log_dir, source))?;
    let mut log = File::create(&log_path).map_err(|source| Error::io(&log_path, source))?;

    let invocation = CodexInvocation::first(&config.codex_bin, &config.root, prompt);
    let (process, stdout) = CodexProcess::spawn(invocation)?;
    let stream_result = stream_codex_output(config, renderer, &log_path, &mut log, stdout);
    process.finish(&config.codex_bin, stream_result)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanningTurnOutput {
    thread_id: Option<String>,
    last_agent_message: Option<String>,
    plan_changed: bool,
}

enum PlanTurn<'a> {
    First,
    Resume { thread_id: &'a str },
}

fn run_planning_turn(
    config: &PlanConfig,
    turn: PlanTurn<'_>,
    prompt: String,
    turn_number: u32,
    plan_before: &PlanSnapshot,
    spinner_text: &'static str,
) -> Result<PlanningTurnOutput, Error> {
    let log_name = format!("{}-plan-{turn_number:03}.jsonl", config.run_stamp);
    let log_path = config.log_dir.join(log_name);

    fs::create_dir_all(&config.log_dir).map_err(|source| Error::io(&config.log_dir, source))?;
    let mut log = File::create(&log_path).map_err(|source| Error::io(&log_path, source))?;

    let require_thread_id = matches!(turn, PlanTurn::First);
    let invocation = match turn {
        PlanTurn::First => CodexInvocation::first(&config.codex_bin, &config.root, prompt),
        PlanTurn::Resume { thread_id } => {
            CodexInvocation::resume(&config.codex_bin, &config.root, thread_id, prompt)
        }
    };

    let (process, stdout) = CodexProcess::spawn(invocation)?;
    let mut spinner =
        Spinner::new(spinner_text).map_err(|source| Error::io("<terminal>", source))?;
    spinner.tick();
    let mut output = PlanningTurnOutput {
        thread_id: None,
        last_agent_message: None,
        plan_changed: false,
    };
    let stream_result =
        stream_planning_output(&log_path, &mut log, stdout, &mut output, &mut spinner);
    let finish_result = process.finish(&config.codex_bin, stream_result);
    spinner.finish();
    finish_result?;

    if require_thread_id && output.thread_id.is_none() {
        return Err(Error::message(
            "codex plan turn completed without thread.started thread_id",
        ));
    }

    let plan_after = PlanSnapshot::capture(&config.plan_abs())?;
    output.plan_changed = &plan_after != plan_before;
    if output.last_agent_message.is_none() && !output.plan_changed {
        return Err(Error::message(
            "codex plan turn completed without an agent message and did not change PLAN.md",
        ));
    }

    Ok(output)
}

struct CodexProcess {
    child: Child,
    stdin_writer: JoinHandle<std::io::Result<()>>,
}

impl CodexProcess {
    fn spawn(invocation: CodexInvocation<'_>) -> Result<(Self, ChildStdout), Error> {
        let mut command = Command::new(invocation.codex_bin);
        command
            .args(invocation.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if invocation.current_dir {
            command.current_dir(invocation.root);
        }

        let mut child = command
            .spawn()
            .map_err(|source| Error::io(invocation.codex_bin, source))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| finish_spawn_error(&mut child, "failed to open codex stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| finish_spawn_error(&mut child, "failed to open codex stdout"))?;
        let prompt = invocation.prompt;

        let stdin_writer = thread::spawn(move || -> std::io::Result<()> {
            stdin.write_all(prompt.as_bytes())?;
            stdin.write_all(b"\n")?;
            Ok(())
        });

        Ok((
            Self {
                child,
                stdin_writer,
            },
            stdout,
        ))
    }

    fn finish(mut self, codex_bin: &str, stream_result: Result<(), Error>) -> Result<(), Error> {
        if stream_result.is_err() {
            let _ = self.child.kill();
        }

        let writer_result = self
            .stdin_writer
            .join()
            .map_err(|_| Error::message("codex stdin writer panicked"))
            .and_then(|result| result.map_err(|source| Error::io("<codex stdin>", source)));

        let status_result = self
            .child
            .wait()
            .map_err(|source| Error::io(codex_bin, source));

        stream_result?;
        writer_result?;

        let status = status_result?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::CodexStatus { status })
        }
    }
}

struct CodexInvocation<'a> {
    codex_bin: &'a str,
    root: &'a Path,
    args: Vec<OsString>,
    prompt: String,
    current_dir: bool,
}

impl<'a> CodexInvocation<'a> {
    fn first(codex_bin: &'a str, root: &'a Path, prompt: String) -> Self {
        Self {
            codex_bin,
            root,
            args: vec![
                "exec".into(),
                "-C".into(),
                root.as_os_str().to_os_string(),
                "--dangerously-bypass-approvals-and-sandbox".into(),
                "--json".into(),
                "-".into(),
            ],
            prompt,
            current_dir: false,
        }
    }

    fn resume(codex_bin: &'a str, root: &'a Path, thread_id: &str, prompt: String) -> Self {
        Self {
            codex_bin,
            root,
            args: vec![
                "exec".into(),
                "resume".into(),
                thread_id.into(),
                "--dangerously-bypass-approvals-and-sandbox".into(),
                "--json".into(),
                "-".into(),
            ],
            prompt,
            current_dir: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PlanSnapshot {
    Missing,
    Present {
        len: u64,
        modified: Option<SystemTime>,
        content: Vec<u8>,
    },
}

impl PlanSnapshot {
    fn capture(path: &Path) -> Result<Self, Error> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::Missing);
            }
            Err(error) => return Err(Error::io(path, error)),
        };
        let content = fs::read(path).map_err(|source| Error::io(path, source))?;
        Ok(Self::Present {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            content,
        })
    }
}

fn finish_spawn_error(child: &mut Child, message: &'static str) -> Error {
    let _ = child.kill();
    let _ = child.wait();
    Error::message(message)
}

fn stream_codex_output(
    config: &Config,
    renderer: &mut Renderer,
    log_path: &std::path::Path,
    log: &mut File,
    stdout: ChildStdout,
) -> Result<(), Error> {
    let result = stream_codex_output_inner(config, renderer, log_path, log, stdout);
    renderer.finish();
    result
}

fn stream_codex_output_inner(
    config: &Config,
    renderer: &mut Renderer,
    log_path: &std::path::Path,
    log: &mut File,
    stdout: ChildStdout,
) -> Result<(), Error> {
    stream_codex_jsonl(log_path, log, stdout, |event| match event {
        JsonlStreamEvent::Idle => {
            renderer.tick();
            Ok(())
        }
        JsonlStreamEvent::Line(line) => match config.stream_mode {
            StreamMode::Raw => {
                std::io::stdout()
                    .write_all(line)
                    .map_err(|source| Error::io("<stdout>", source))?;
                Ok(())
            }
            StreamMode::Pretty => {
                let line_str = String::from_utf8_lossy(line);
                match CodexEvent::parse(&line_str) {
                    Ok(event) => renderer.event(&event),
                    Err(error) => renderer.raw_parse_error(&line_str, &error),
                }
                Ok(())
            }
        },
    })
}

fn stream_planning_output(
    log_path: &Path,
    log: &mut File,
    stdout: ChildStdout,
    output: &mut PlanningTurnOutput,
    spinner: &mut Spinner,
) -> Result<(), Error> {
    stream_codex_jsonl(log_path, log, stdout, |event| {
        let line = match event {
            JsonlStreamEvent::Idle => {
                spinner.tick();
                return Ok(());
            }
            JsonlStreamEvent::Line(line) => line,
        };
        let line_str = String::from_utf8_lossy(line);
        if let Ok(event) = CodexEvent::parse(&line_str) {
            match event.payload {
                EventPayload::ThreadStarted { thread_id } => {
                    output.thread_id = Some(thread_id);
                }
                EventPayload::Item { item } => {
                    if let ItemPayload::AgentMessage { text } = item.payload {
                        output.last_agent_message = Some(text);
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })
}

fn stream_codex_jsonl(
    log_path: &Path,
    log: &mut File,
    stdout: ChildStdout,
    mut on_event: impl FnMut(JsonlStreamEvent<'_>) -> Result<(), Error>,
) -> Result<(), Error> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.split(b'\n') {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    loop {
        let mut line = match rx.recv_timeout(Duration::from_millis(120)) {
            Ok(line) => line.map_err(|source| Error::io(log_path, source))?,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                on_event(JsonlStreamEvent::Idle)?;
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if line.is_empty() {
            continue;
        }
        line.push(b'\n');
        log.write_all(&line)
            .map_err(|source| Error::io(log_path, source))?;
        on_event(JsonlStreamEvent::Line(&line))?;
    }
    Ok(())
}

enum JsonlStreamEvent<'a> {
    Idle,
    Line(&'a [u8]),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    #[test]
    fn finish_kills_child_when_streaming_fails() {
        let child = Command::new("sh")
            .args(["-c", "sleep 60"])
            .spawn()
            .expect("spawn child");
        let process = CodexProcess {
            child,
            stdin_writer: thread::spawn(|| Ok(())),
        };

        let error = process
            .finish("sh", Err(Error::message("stream failed")))
            .expect_err("stream error should win");

        assert_eq!(error.to_string(), "stream failed");
    }

    #[test]
    fn run_plan_rejects_unmanaged_lgtm_skill_before_git_init() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        fs::write(root.join("PLAN.md"), "# Plan\n\n## Phase 1: Test\n").expect("plan");
        fs::write(root.join("AGENTS.md"), "# Agents\n").expect("agents");

        let skill_dir = root
            .join(".agents")
            .join("skills")
            .join(skills::PHASE_IMPLEMENT);
        fs::create_dir_all(&skill_dir).expect("skill dir");
        fs::write(skill_dir.join("SKILL.md"), "team owned").expect("skill");

        let config = Config {
            root: root.to_path_buf(),
            plan_path: "PLAN.md".into(),
            agents_path: "AGENTS.md".into(),
            start_phase: 1,
            end_phase: Some(1),
            sleep_seconds: 0,
            codex_bin: "codex".to_string(),
            stream_mode: StreamMode::Pretty,
            log_dir: root.join(".codex-log"),
            run_stamp: "test".to_string(),
        };

        let error = run_plan(config).expect_err("unmanaged skill should abort");

        assert!(error.to_string().contains("is not managed by lgtm"));
        assert!(!root.join(".git").exists());
    }

    #[test]
    fn planning_turn_captures_thread_id_last_message_and_log() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let fake_codex = executable(
            temp.path(),
            r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
printf '%s\n' "$*" >"$dir/args.txt"
pwd >"$dir/cwd.txt"
cat >"$dir/stdin.txt"
printf '%s\n' '{"type":"thread.started","thread_id":"thread-test"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"first question"}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"second question"}}'
"#,
        );
        let config = plan_config(&root, fake_codex);
        let snapshot = PlanSnapshot::capture(&config.plan_abs()).expect("snapshot");

        let output = run_planning_turn(
            &config,
            PlanTurn::First,
            "planning prompt".to_string(),
            1,
            &snapshot,
            "test spinner",
        )
        .expect("planning turn");

        assert_eq!(output.thread_id.as_deref(), Some("thread-test"));
        assert_eq!(
            output.last_agent_message.as_deref(),
            Some("second question")
        );
        assert!(!output.plan_changed);
        assert!(
            fs::read_to_string(temp.path().join("args.txt"))
                .expect("args")
                .contains("exec -C")
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("stdin.txt")).expect("stdin"),
            "planning prompt\n"
        );
        let log = fs::read_to_string(root.join(".codex-log/test-plan-001.jsonl")).expect("log");
        assert!(log.contains("thread-test"));
        assert!(log.contains("second question"));
    }

    #[test]
    fn planning_resume_uses_explicit_thread_id_without_last() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let fake_codex = executable(
            temp.path(),
            r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
printf '%s\n' "$*" >"$dir/args.txt"
pwd >"$dir/cwd.txt"
cat >/dev/null
printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"resumed answer"}}'
"#,
        );
        let config = plan_config(&root, fake_codex);
        let snapshot = PlanSnapshot::capture(&config.plan_abs()).expect("snapshot");

        let output = run_planning_turn(
            &config,
            PlanTurn::Resume {
                thread_id: "thread-test",
            },
            "user answer".to_string(),
            2,
            &snapshot,
            "test spinner",
        )
        .expect("resume turn");

        assert_eq!(output.last_agent_message.as_deref(), Some("resumed answer"));
        assert!(!output.plan_changed);
        let args = fs::read_to_string(temp.path().join("args.txt")).expect("args");
        assert!(args.contains("exec resume thread-test"));
        assert!(!args.contains("--last"));
        assert_eq!(
            fs::canonicalize(
                fs::read_to_string(temp.path().join("cwd.txt"))
                    .expect("cwd")
                    .trim()
            )
            .expect("canonical cwd"),
            fs::canonicalize(&root).expect("canonical root")
        );
        assert!(root.join(".codex-log/test-plan-002.jsonl").is_file());
    }

    #[test]
    fn first_planning_turn_requires_thread_id() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let fake_codex = executable(
            temp.path(),
            r#"#!/usr/bin/env sh
set -eu
cat >/dev/null
printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"question"}}'
"#,
        );
        let config = plan_config(&root, fake_codex);
        let snapshot = PlanSnapshot::capture(&config.plan_abs()).expect("snapshot");

        let error = run_planning_turn(
            &config,
            PlanTurn::First,
            "planning prompt".to_string(),
            1,
            &snapshot,
            "test spinner",
        )
        .expect_err("missing thread id should fail");

        assert!(
            error
                .to_string()
                .contains("completed without thread.started thread_id")
        );
    }

    #[test]
    fn planning_turn_requires_message_before_plan_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let fake_codex = executable(
            temp.path(),
            r#"#!/usr/bin/env sh
set -eu
cat >/dev/null
printf '%s\n' '{"type":"thread.started","thread_id":"thread-test"}'
"#,
        );
        let config = plan_config(&root, fake_codex);
        let snapshot = PlanSnapshot::capture(&config.plan_abs()).expect("snapshot");

        let error = run_planning_turn(
            &config,
            PlanTurn::First,
            "planning prompt".to_string(),
            1,
            &snapshot,
            "test spinner",
        )
        .expect_err("missing message should fail");

        assert!(
            error
                .to_string()
                .contains("completed without an agent message and did not change PLAN.md")
        );
    }

    #[test]
    fn planning_turn_allows_missing_message_when_plan_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let fake_codex = executable(
            temp.path(),
            r#"#!/usr/bin/env sh
set -eu
cat >/dev/null
printf '%s\n' '{"type":"thread.started","thread_id":"thread-test"}'
cat >"$3/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Done

Goal: Done.

Steps:

- Done.

Validation:

- Done.
PLAN
"#,
        );
        let config = plan_config(&root, fake_codex);
        let snapshot = PlanSnapshot::capture(&config.plan_abs()).expect("snapshot");

        let output = run_planning_turn(
            &config,
            PlanTurn::First,
            "planning prompt".to_string(),
            1,
            &snapshot,
            "test spinner",
        )
        .expect("plan completion without message should be accepted");

        assert_eq!(output.thread_id.as_deref(), Some("thread-test"));
        assert_eq!(output.last_agent_message, None);
        assert!(output.plan_changed);
        assert!(root.join("PLAN.md").is_file());
    }

    fn plan_config(root: &Path, codex_bin: PathBuf) -> PlanConfig {
        PlanConfig {
            root: root.to_path_buf(),
            plan_path: "PLAN.md".into(),
            brief: None,
            codex_bin: codex_bin.display().to_string(),
            log_dir: root.join(".codex-log"),
            run_stamp: "test".to_string(),
        }
    }

    fn executable(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("codex");
        fs::write(&path, body).expect("write fake codex");
        let mut perms = fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).expect("chmod");
        path
    }
}
