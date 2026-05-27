use std::{
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{fs::PermissionsExt, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn plan_mode_quit_exits_after_first_question_in_tty() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("agents");

    let fake_codex = executable(
        temp.path(),
        r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"

read initialize
printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
read initialized
read thread_start
printf '%s\n' '{"id":2,"result":{"thread":{"id":"thr-plan"}}}'
read turn_start
printf '%s\n' "$turn_start" >"$dir/turn-$n.json"
printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-plan","status":"inProgress","items":[]}}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"completed","items":[{"type":"agentMessage","id":"msg-1","text":"**Pick one**\n\n- Option A","status":"completed"}]}}}'
"#,
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test");

    let output = run_with_pty(command, "/quit\r");
    let plain_stdout = strip_ansi_escape_sequences(&output.stdout);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(plain_stdout.contains(">_ lgtm"));
    assert!(plain_stdout.contains("mode:        plan"));
    assert!(plain_stdout.contains("execution:   host YOLO"));
    assert!(plain_stdout.contains("Pick one"));
    assert!(plain_stdout.contains("Option A"));
    assert!(plain_stdout.contains("> /quit"));
    assert_eq!(
        fs::read_to_string(temp.path().join("counter")).expect("counter"),
        "1\n"
    );
    assert!(repo.join(".lgtm/logs/test-plan-001.jsonl").is_file());
    assert!(!temp.path().join("turn-2.json").exists());
}

#[test]
fn plan_mode_prompts_for_next_step_after_final_plan_is_written() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("agents");

    let fake_codex = executable(temp.path(), completed_plan_codex_script());

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test");

    let output = run_with_pty(command, "exit\r");
    let plain_stdout = strip_ansi_escape_sequences(&output.stdout);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(repo.join("PLAN.md").is_file());
    assert!(plain_stdout.contains("final plan written"));
    assert!(plain_stdout.contains("Plan artifacts are ready."));
    assert!(plain_stdout.contains("Implement now or exit? [i/e]"));
    assert!(plain_stdout.contains("> exit"));
}

#[test]
fn plan_mode_empty_post_plan_choice_is_invalid_and_reprompts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("agents");

    let fake_codex = executable(temp.path(), completed_plan_codex_script());

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test");

    let output = run_with_pty(command, "\re\r");
    let plain_stdout = strip_ansi_escape_sequences(&output.stdout);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(plain_stdout.contains("Invalid choice. Enter implement or exit."));
    assert_eq!(
        plain_stdout.matches("Implement now or exit? [i/e]").count(),
        2
    );
    assert!(plain_stdout.contains("> e"));
    assert_eq!(
        fs::read_to_string(temp.path().join("counter")).expect("counter"),
        "1\n"
    );
}

#[test]
fn plan_mode_exit_choice_stops_after_planning_without_invoking_run_mode() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("agents");

    let fake_codex = executable(temp.path(), completed_plan_codex_script());

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test");

    let output = run_with_pty(command, "n\r");
    let plain_stdout = strip_ansi_escape_sequences(&output.stdout);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(plain_stdout.contains("Implement now or exit? [i/e]"));
    assert!(plain_stdout.contains("> n"));
    assert_eq!(
        fs::read_to_string(temp.path().join("counter")).expect("counter"),
        "1\n"
    );
    assert!(repo.join(".lgtm/logs/test-plan-001.jsonl").is_file());
    assert!(!temp.path().join("turn-2.json").exists());
}

#[test]
fn plan_mode_implement_choice_hands_off_to_run_mode() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("agents");

    let fake_codex = executable(temp.path(), immediate_handoff_codex_script());

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--log-dir")
        .arg("handoff-logs")
        .arg("--run-stamp")
        .arg("test");

    let output = run_with_pty(command, "i\r");
    let plain_stdout = strip_ansi_escape_sequences(&output.stdout);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(plain_stdout.contains("Plan artifacts are ready."));
    assert!(plain_stdout.contains("> i"));
    assert!(plain_stdout.contains("mode:        run"));
    assert!(plain_stdout.contains("• Phase 01 implementation: Generated"));
    assert!(plain_stdout.contains("• Phase 01 validation: Generated"));
    assert!(plain_stdout.contains("• Phase 01 review: Generated"));
    assert!(plain_stdout.contains("• Phase 01 commit: Generated"));
    assert_eq!(
        fs::read_to_string(temp.path().join("counter")).expect("counter"),
        "7\n"
    );

    assert!(repo.join("handoff-logs/test-plan-001.jsonl").is_file());
    assert!(
        repo.join("handoff-logs/test-phase-01-index.jsonl")
            .is_file()
    );
    assert!(
        repo.join("handoff-logs/test-phase-01-implement.jsonl")
            .is_file()
    );
    assert!(
        repo.join("handoff-logs/test-phase-01-validate.jsonl")
            .is_file()
    );
    assert!(
        repo.join("handoff-logs/test-phase-01-review.jsonl")
            .is_file()
    );
    assert!(
        repo.join("handoff-logs/test-phase-01-commit.jsonl")
            .is_file()
    );
    assert!(
        repo.join("handoff-logs/test-phase-02-index.jsonl")
            .is_file()
    );

    let index_turn = fs::read_to_string(temp.path().join("turn-2.json")).expect("index prompt");
    let implement_turn =
        fs::read_to_string(temp.path().join("turn-3.json")).expect("implement prompt");
    let validate_turn =
        fs::read_to_string(temp.path().join("turn-4.json")).expect("validate prompt");
    let review_turn = fs::read_to_string(temp.path().join("turn-5.json")).expect("review prompt");
    let commit_turn = fs::read_to_string(temp.path().join("turn-6.json")).expect("commit prompt");
    assert!(index_turn.contains("# Plan"));
    assert!(index_turn.contains("## Phase 1 - Generated"));
    assert!(index_turn.contains("Goal: generated."));
    assert!(implement_turn.contains("$lgtm-phase-implement"));
    assert!(implement_turn.contains("## Phase 1 - Generated"));
    assert!(validate_turn.contains("$lgtm-phase-validate"));
    assert!(review_turn.contains("$lgtm-phase-review"));
    assert!(commit_turn.contains("$lgtm-phase-commit"));
    assert!(commit_turn.contains("## Phase 1 - Generated"));
}

#[test]
fn plan_mode_implement_choice_propagates_run_mode_failure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("agents");

    let fake_codex = executable(temp.path(), failing_handoff_codex_script());

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--log-dir")
        .arg("handoff-logs")
        .arg("--run-stamp")
        .arg("test");

    let output = run_with_pty(command, "implement\r");
    let plain_stdout = strip_ansi_escape_sequences(&output.stdout);

    assert!(
        !output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(plain_stdout.contains("Plan artifacts are ready."));
    assert!(plain_stdout.contains("> implement"));
    assert!(plain_stdout.contains("mode:        run"));
    assert!(plain_stdout.contains("• Phase 01 implementation: Generated"));
    assert!(plain_stdout.contains("turn failed: implementation failed"));
    assert_eq!(
        fs::read_to_string(temp.path().join("counter")).expect("counter"),
        "3\n"
    );
    assert!(repo.join("handoff-logs/test-plan-001.jsonl").is_file());
    assert!(
        repo.join("handoff-logs/test-phase-01-index.jsonl")
            .is_file()
    );
    assert!(
        repo.join("handoff-logs/test-phase-01-implement.jsonl")
            .is_file()
    );
    assert!(
        !repo
            .join("handoff-logs/test-phase-01-validate.jsonl")
            .exists()
    );
    assert!(
        !repo
            .join("handoff-logs/test-phase-01-review.jsonl")
            .exists()
    );
    assert!(
        !repo
            .join("handoff-logs/test-phase-01-commit.jsonl")
            .exists()
    );
}

struct PtyOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_with_pty(mut command: Command, input: &str) -> PtyOutput {
    let (master, slave) = open_pty();
    let child_stdin = slave.try_clone().expect("clone slave stdin");
    let child_stdout = slave.try_clone().expect("clone slave stdout");
    let child_stderr = slave;

    let mut child = unsafe {
        command
            .stdin(Stdio::from(child_stdin))
            .stdout(Stdio::from(child_stdout))
            .stderr(Stdio::from(child_stderr))
            .pre_exec(|| {
                // Put the child in its own session so the PTY is its controlling terminal.
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            })
            .spawn()
            .expect("spawn child")
    };

    set_nonblocking(&master);
    let mut stdout = Vec::new();
    wait_for_pty_prompt(&mut child, &master, &mut stdout);
    (&master)
        .write_all(input.as_bytes())
        .expect("write pty input");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        read_available_pty_output(&master, &mut stdout);
        if let Some(status) = child.try_wait().expect("poll child") {
            read_available_pty_output(&master, &mut stdout);
            return PtyOutput {
                status,
                stdout: String::from_utf8_lossy(&stdout).to_string(),
                stderr: String::new(),
            };
        }
        assert!(Instant::now() < deadline, "child timed out");
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_pty_prompt(child: &mut std::process::Child, master: &File, stdout: &mut Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        read_available_pty_output(master, stdout);
        if String::from_utf8_lossy(stdout).contains("> ") {
            return;
        }
        if let Some(status) = child.try_wait().expect("poll child") {
            panic!(
                "child exited before prompt: {status}\n{}",
                String::from_utf8_lossy(stdout)
            );
        }
        assert!(Instant::now() < deadline, "timed out waiting for prompt");
        thread::sleep(Duration::from_millis(20));
    }
}

fn read_available_pty_output(master: &File, stdout: &mut Vec<u8>) {
    let mut buffer = [0_u8; 4096];
    loop {
        let mut reader = master;
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => stdout.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("read pty: {error}"),
        }
    }
}

fn set_nonblocking(file: &File) {
    unsafe {
        let flags = libc::fcntl(file.as_raw_fd(), libc::F_GETFL);
        assert!(flags >= 0, "fcntl F_GETFL failed");
        let result = libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
        assert!(result >= 0, "fcntl F_SETFL failed");
    }
}

fn strip_ansi_escape_sequences(output: &str) -> String {
    let mut stripped = String::with_capacity(output.len());
    let mut chars = output.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            stripped.push(ch);
            continue;
        }

        if chars.next_if_eq(&'[').is_none() {
            stripped.push(ch);
            continue;
        }

        for ch in chars.by_ref() {
            if ('@'..='~').contains(&ch) {
                break;
            }
        }
    }

    stripped
}

fn open_pty() -> (File, File) {
    let mut master = 0;
    let mut slave = 0;
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(result, 0, "openpty failed");

    unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) }
}

fn init_git_repo(repo: &Path) {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("init")
        .output()
        .expect("git init");
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["branch", "-M", "main"])
        .output()
        .expect("git branch");
}

fn completed_plan_codex_script() -> &'static str {
    r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"

read initialize
printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
read initialized
read thread_start
printf '%s\n' '{"id":2,"result":{"thread":{"id":"thr-plan"}}}'
read turn_start
printf '%s\n' "$turn_start" >"$dir/turn-$n.json"
cat >"$dir/repo/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Test

Goal: test.
PLAN
printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-plan","status":"inProgress","items":[]}}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"completed","items":[{"type":"agentMessage","id":"msg-1","text":"final plan written","status":"completed"}]}}}'
"#
}

fn immediate_handoff_codex_script() -> &'static str {
    r###"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"

read initialize
printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
read initialized
read thread_start
printf '%s\n' '{"id":2,"result":{"thread":{"id":"thr-plan"}}}'
read turn_start
printf '%s\n' "$turn_start" >"$dir/turn-$n.json"
printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-plan","status":"inProgress","items":[]}}}'

if [ "$n" = 1 ]; then
  cat >"$dir/repo/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Generated

Goal: generated.
PLAN
  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"completed","items":[{"type":"agentMessage","id":"msg-plan","text":"final plan written","status":"completed"}]}}}'
elif [ "$n" = 2 ] || [ "$n" = 7 ]; then
  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"completed","items":[{"type":"agentMessage","id":"msg-index","text":"{\"phases\":[{\"id\":1,\"title\":\"Generated\",\"heading\":\"## Phase 1 - Generated\"}]}","status":"completed"}]}}}'
else
  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"completed","items":[{"type":"agentMessage","id":"msg-pass","text":"done","status":"completed"}]}}}'
fi
"###
}

fn failing_handoff_codex_script() -> &'static str {
    r###"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"

read initialize
printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
read initialized
read thread_start
printf '%s\n' '{"id":2,"result":{"thread":{"id":"thr-plan"}}}'
read turn_start
printf '%s\n' "$turn_start" >"$dir/turn-$n.json"
printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-plan","status":"inProgress","items":[]}}}'

if [ "$n" = 1 ]; then
  cat >"$dir/repo/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Generated

Goal: generated.
PLAN
  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"completed","items":[{"type":"agentMessage","id":"msg-plan","text":"final plan written","status":"completed"}]}}}'
elif [ "$n" = 2 ]; then
  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"completed","items":[{"type":"agentMessage","id":"msg-index","text":"{\"phases\":[{\"id\":1,\"title\":\"Generated\",\"heading\":\"## Phase 1 - Generated\"}]}","status":"completed"}]}}}'
else
  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"failed","error":{"message":"implementation failed"},"items":[{"type":"agentMessage","id":"msg-pass","text":"failed","status":"completed"}]}}}'
fi
"###
}

fn executable(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("codex");
    fs::write(&path, body).expect("script");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod");
    path
}
