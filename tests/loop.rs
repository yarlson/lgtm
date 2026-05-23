use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::FromRawFd;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

#[test]
fn runs_implementation_validation_and_review_prompts_with_formatted_output() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("init")
        .output()
        .expect("git init");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["branch", "-M", "main"])
        .output()
        .expect("rename branch");
    fs::write(
        repo.join("PLAN.md"),
        "# Plan\n\n## Phase 1: Skeleton\n\nGoal: test.\n",
    )
    .expect("write plan");
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("write agents");

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
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
cat >"$dir/prompt-$n.txt"
printf '%s\n' '{"type":"thread.started","thread_id":"thread-test"}'
printf '%s\n' '{"type":"turn.started"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"done"}}'
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":2,"reasoning_output_tokens":0}}'
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("run")
        .arg("--root")
        .arg(&repo)
        .arg("--end-phase")
        .arg("1")
        .arg("--sleep-seconds")
        .arg("0")
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test")
        .output()
        .expect("run lgtm");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("phase=01"));
    assert!(stdout.contains("pass=review"));
    assert!(stdout.contains("• Ran thread thread-test"));
    assert!(stdout.contains("• Ran turn begin"));
    assert!(stdout.contains("• Codex"));
    assert!(stdout.contains("  done"));
    assert!(stdout.contains("• Verification tokens input=1 cached=0 output=2 reasoning=0"));

    let implement_prompt =
        fs::read_to_string(temp.path().join("prompt-1.txt")).expect("implementation prompt");
    let validate_prompt =
        fs::read_to_string(temp.path().join("prompt-2.txt")).expect("validation prompt");
    let review_prompt =
        fs::read_to_string(temp.path().join("prompt-3.txt")).expect("review prompt");
    for prompt in [&implement_prompt, &validate_prompt, &review_prompt] {
        assert!(prompt.contains("## Phase 1: Skeleton"));
        assert!(!prompt.contains("## Phase 1 - Skeleton"));
    }
    assert!(implement_prompt.contains("$lgtm-phase-implement"));
    assert!(validate_prompt.contains("$lgtm-phase-validate"));
    assert!(review_prompt.contains("$lgtm-phase-review"));

    let default_logs = repo.join(".codex-log");
    assert!(default_logs.join("test-phase-01-implement.jsonl").is_file());
    assert!(default_logs.join("test-phase-01-validate.jsonl").is_file());
    assert!(default_logs.join("test-phase-01-review.jsonl").is_file());
    assert!(
        repo.join(".agents")
            .join("skills")
            .join("lgtm-phase-implement")
            .join("SKILL.md")
            .is_file()
    );
    assert!(
        repo.join(".agents")
            .join("skills")
            .join("lgtm-phase-review")
            .join("SKILL.md")
            .is_file()
    );
    assert!(
        fs::read_to_string(repo.join(".gitignore"))
            .expect("read gitignore")
            .lines()
            .any(|line| line == ".agents/skills/lgtm-*")
    );
    assert!(
        fs::read_to_string(repo.join(".gitignore"))
            .expect("read gitignore")
            .lines()
            .any(|line| line == ".codex-log/")
    );
}

#[test]
fn reloads_plan_before_each_phase() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .arg("init")
        .output()
        .expect("git init");
    Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["branch", "-M", "main"])
        .output()
        .expect("rename branch");
    fs::write(
        repo.join("PLAN.md"),
        "# Plan\n\n## Phase 1: One\n\n## Phase 2: Old Two\n",
    )
    .expect("write plan");
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("write agents");

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
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
if [ "$n" = 1 ]; then
  printf '# Plan\n\n## Phase 1: One\n\n## Phase 2: Changed Two\n' >"$LGTM_TEST_REPO/PLAN.md"
fi
cat >/dev/null
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1}}'
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("run")
        .arg("--root")
        .arg(&repo)
        .arg("--end-phase")
        .arg("2")
        .arg("--sleep-seconds")
        .arg("0")
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test")
        .env("LGTM_TEST_REPO", &repo)
        .output()
        .expect("run lgtm");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("phase=02 pass=implement title=\"Changed Two\""));
    assert!(!stdout.contains("phase=02 pass=implement title=\"Old Two\""));
}

#[test]
fn plan_first_turn_prints_only_last_agent_message_and_writes_log() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
printf '%s\n' "$*" >"$dir/plan-args.txt"
cat >"$dir/plan-prompt.txt"
printf '%s\n' '{"type":"thread.started","thread_id":"thread-plan"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"first question"}}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"second question"}}'
cat >"$LGTM_TEST_REPO/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Done

Goal: Done.

Steps:

- Done.

Validation:

- Done.
PLAN
"#,
    )
    .expect("write fake codex");
    let mut perms = fs::metadata(&fake_codex)
        .expect("fake metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_codex, perms).expect("chmod fake codex");

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("ship smaller phases")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test")
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(output.stdout.contains("second question"));
    assert!(!output.stdout.contains("first question"));
    assert!(!output.stderr.contains("thread-plan"));

    let args = fs::read_to_string(temp.path().join("plan-args.txt")).expect("args");
    assert!(args.contains("exec -C"));
    assert!(args.contains("--json -"));

    let prompt = fs::read_to_string(temp.path().join("plan-prompt.txt")).expect("prompt");
    assert!(prompt.contains("$lgtm-plan-create"));
    assert!(prompt.contains("User brief:\nship smaller phases"));

    let log = fs::read_to_string(repo.join(".codex-log/test-plan-001.jsonl")).expect("log");
    assert!(log.contains("thread-plan"));
    assert!(log.contains("first question"));
    assert!(log.contains("second question"));
    assert!(!repo.join("AGENTS.md").exists());
    assert!(repo.join("PLAN.md").is_file());
}

#[test]
fn plan_mode_rejects_non_tty_before_preflight() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg("codex-never-started")
        .output()
        .expect("run lgtm plan");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires interactive stdin and stdout"));
    assert!(!repo.join(".git").exists());
}

#[test]
fn plan_mode_rejects_unmanaged_lgtm_skill_before_codex_starts() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);
    let skill_dir = repo.join(".agents").join("skills").join("lgtm-team-owned");
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(skill_dir.join("SKILL.md"), "team owned").expect("write unmanaged skill");

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
touch "$LGTM_TEST_REPO/codex-started"
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "");

    assert!(!output.status.success());
    assert!(output.stderr.contains("is not managed by lgtm"));
    assert!(!repo.join("codex-started").exists());
}

#[test]
fn plan_mode_loops_until_codex_creates_plan() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/plan-counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"
cat >"$dir/plan-prompt-$n.txt"
if [ "$n" = 1 ]; then
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-plan"}'
  printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Choose **A** or **B**?\n\n"}}'
else
  printf '%s\n' '{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Plan written."}}'
  cat >"$LGTM_TEST_REPO/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Done

Goal: Done.

Steps:

- Done.

Validation:

- Done.
PLAN
fi
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test")
        .env("NO_COLOR", "1")
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "Line one\nLine two\r");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("plan-counter")).expect("counter"),
        "2\n"
    );
    let stdout = stable_plan_stdout(&output.stdout);
    assert!(stdout.starts_with("Choose A or B?\n"), "stdout:\n{stdout}");
    assert!(
        !stdout.starts_with("Choose A or B?\n\n"),
        "stdout:\n{stdout}"
    );
    assert!(!stdout.contains("**A**"));
    assert!(repo.join("PLAN.md").is_file());
    assert!(repo.join(".codex-log/test-plan-001.jsonl").is_file());
    assert!(repo.join(".codex-log/test-plan-002.jsonl").is_file());
    assert_eq!(
        fs::read_to_string(temp.path().join("plan-prompt-2.txt")).expect("resume prompt"),
        "Line one\nLine two\n"
    );
}

#[test]
fn plan_mode_ctrl_c_clears_input_and_second_quick_ctrl_c_quits() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/plan-counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"
cat >"$dir/plan-prompt-$n.txt"
if [ "$n" = 2 ]; then
  cat >"$LGTM_TEST_REPO/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Done

Goal: Done.

Steps:

- Done.

Validation:

- Done.
PLAN
else
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-plan"}'
  printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Answer?"}}'
fi
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "old\nvalue\x03replacement\r");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("plan-counter")).expect("counter"),
        "2\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("plan-prompt-2.txt")).expect("resume prompt"),
        "replacement\n"
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "\x03\x03");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("plan-counter")).expect("counter"),
        "3\n"
    );
    assert!(!temp.path().join("plan-prompt-4.txt").exists());
}

#[test]
fn plan_mode_shows_spinner_while_waiting_for_codex() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
cat >/dev/null
sleep 1
printf '%s\n' '{"type":"thread.started","thread_id":"thread-plan"}'
cat >"$LGTM_TEST_REPO/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Done

Goal: Done.

Steps:

- Done.

Validation:

- Done.
PLAN
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(
        contains_italic_spinner_frame(&output.stdout, "."),
        "stdout:\n{}",
        output.stdout
    );
    assert!(
        contains_italic_spinner_frame(&output.stdout, ".."),
        "stdout:\n{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("\u{1b}[?25l"),
        "stdout:\n{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("\u{1b}[?25h"),
        "stdout:\n{}",
        output.stdout
    );
}

#[test]
fn plan_mode_restores_terminal_on_ctrl_c_while_spinner_active() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
cat >/dev/null
sleep 1
kill -INT "$PPID"
sleep 1
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "");

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(130));
    assert!(
        output.stdout.contains("\u{1b}[?25l"),
        "stdout:\n{}",
        output.stdout
    );
    assert!(
        output.stdout.contains("\r\u{1b}[2K\u{1b}[?25h"),
        "stdout:\n{}",
        output.stdout
    );
}

#[test]
fn plan_mode_bracketed_paste_preserves_multiline_answer() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/plan-counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"
cat >"$dir/plan-prompt-$n.txt"
if [ "$n" = 1 ]; then
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-plan"}'
  printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Paste context."}}'
else
  cat >"$LGTM_TEST_REPO/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Done

Goal: Done.

Steps:

- Done.

Validation:

- Done.
PLAN
fi
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "\x1b[200~one\r\ntwo\rthree\x1b[201~\r");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("plan-prompt-2.txt")).expect("resume prompt"),
        "one\ntwo\nthree\n"
    );
}

#[test]
fn plan_mode_stops_when_codex_modifies_existing_plan() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);
    fs::write(repo.join("PLAN.md"), "# Plan\n\n## Phase 1 - Old\n").expect("write plan");

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/plan-counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"
cat >/dev/null
printf '%s\n' '{"type":"thread.started","thread_id":"thread-plan"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Plan updated."}}'
printf '%s\n' '# Plan\n\n## Phase 1 - New\n' >"$LGTM_TEST_REPO/PLAN.md"
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("plan-counter")).expect("counter"),
        "1\n"
    );
    assert!(
        fs::read_to_string(repo.join("PLAN.md"))
            .expect("plan")
            .contains("Phase 1 - New")
    );
}

#[test]
fn plan_mode_quit_exits_without_resume_turn() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/plan-counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"
cat >/dev/null
printf '%s\n' '{"type":"thread.started","thread_id":"thread-plan"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Question?"}}'
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "/quit\r");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("plan-counter")).expect("counter"),
        "1\n"
    );
    assert!(!repo.join("PLAN.md").exists());
}

#[test]
fn plan_mode_finish_sends_finalization_prompt() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("create repo");
    init_git_repo(&repo);

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
counter="$dir/plan-counter"
if [ -f "$counter" ]; then
  n=$(cat "$counter")
else
  n=0
fi
n=$((n + 1))
printf '%s\n' "$n" >"$counter"
cat >"$dir/plan-prompt-$n.txt"
if [ "$n" = 1 ]; then
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-plan"}'
  printf '%s\n' '{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"Need anything else?"}}'
else
  cat >"$LGTM_TEST_REPO/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Done

Goal: Done.

Steps:

- Done.

Validation:

- Done.
PLAN
fi
"#,
    )
    .expect("write fake codex");
    make_executable(&fake_codex);

    let mut command = Command::new(env!("CARGO_BIN_EXE_lgtm"));
    command
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .env("LGTM_TEST_REPO", &repo);
    let output = run_with_pty(command, "/finish\r");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    let resume_prompt =
        fs::read_to_string(temp.path().join("plan-prompt-2.txt")).expect("resume prompt");
    assert!(resume_prompt.contains("user requested /finish"));
    assert!(resume_prompt.contains("unresolved risks"));
    assert!(resume_prompt.contains("do not invent certainty"));
    assert!(!resume_prompt.contains("ask exactly one remaining sharp question"));
}

fn init_git_repo(repo: &std::path::Path) {
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
        .expect("rename branch");
}

fn make_executable(path: &std::path::Path) {
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

fn stable_plan_stdout(stdout: &str) -> String {
    let mut stable = stdout.replace("\r\n", "\n");
    for frame in ["...", "..", "."] {
        stable = stable.replace(&format!("\r\x1b[2Kworking {frame}"), "");
        stable = stable.replace(&format!("\r\x1b[2K\x1b[3mworking\x1b[0m {frame}"), "");
    }
    while let Some(start) = stable.find("\r\x1b[2K\x1b[3m") {
        let Some(reset) = stable[start..].find("\x1b[0m ") else {
            break;
        };
        let dots_start = start + reset + "\x1b[0m ".len();
        let dots_len = stable[dots_start..]
            .chars()
            .take_while(|value| *value == '.')
            .count();
        if dots_len == 0 {
            break;
        }
        stable.replace_range(start..dots_start + dots_len, "");
    }
    while let Some(start) = stable.find("\r\x1b[2K\x1b[3;37m") {
        let Some(reset) = stable[start..].find("\x1b[0m \x1b[37m") else {
            break;
        };
        let dots_start = start + reset + "\x1b[0m \x1b[37m".len();
        let dots_len = stable[dots_start..]
            .chars()
            .take_while(|value| *value == '.')
            .count();
        if dots_len == 0 {
            break;
        }
        let reset_len = if stable[dots_start + dots_len..].starts_with("\x1b[0m") {
            "\x1b[0m".len()
        } else {
            0
        };
        stable.replace_range(start..dots_start + dots_len + reset_len, "");
    }
    stable = stable.replace("\x1b[?25l", "");
    stable = stable.replace("\x1b[?25h", "");
    stable.replace("\r\x1b[2K", "")
}

fn contains_italic_spinner_frame(stdout: &str, frame: &str) -> bool {
    let suffix = format!("\x1b[0m \x1b[37m{frame}\x1b[0m");
    stdout.split("\r\x1b[2K\x1b[3;37m").skip(1).any(|part| {
        let Some(end) = part.find(&suffix) else {
            return false;
        };
        let label = &part[..end];
        !label.is_empty()
            && label != "working"
            && label
                .chars()
                .all(|value| value.is_ascii_lowercase() || value == '-')
    })
}

struct PtyOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn run_with_pty(mut command: Command, input: &str) -> PtyOutput {
    use std::ffi::c_void;
    use std::os::raw::c_char;
    use std::os::raw::c_int;

    unsafe extern "C" {
        fn openpty(
            amaster: *mut c_int,
            aslave: *mut c_int,
            name: *mut c_char,
            termp: *const c_void,
            winp: *const c_void,
        ) -> c_int;
    }

    let mut master = 0;
    let mut slave = 0;
    let result = unsafe {
        openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    assert_eq!(
        result,
        0,
        "openpty failed: {}",
        std::io::Error::last_os_error()
    );

    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    command
        .stdin(Stdio::from(slave.try_clone().expect("clone pty stdin")))
        .stdout(Stdio::from(slave.try_clone().expect("clone pty stdout")))
        .stderr(Stdio::piped());

    let mut child = command.spawn().expect("spawn command in pty");
    drop(slave);
    set_nonblocking(&master);

    let mut stdout = Vec::new();
    if !input.is_empty() {
        wait_for_pty_prompt(&mut child, &mut master, &mut stdout);
        master.write_all(input.as_bytes()).expect("write pty input");
    }

    let status = child.wait().expect("wait for command");
    read_available_pty_output(&mut master, &mut stdout);

    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr pipe")
        .read_to_string(&mut stderr)
        .expect("read stderr");

    PtyOutput {
        status,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr,
    }
}

fn wait_for_pty_prompt(child: &mut std::process::Child, master: &mut File, stdout: &mut Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        read_available_pty_output(master, stdout);
        if String::from_utf8_lossy(stdout).contains("> ") {
            return;
        }
        if let Some(status) = child.try_wait().expect("poll child") {
            panic!("command exited before prompt: {status}");
        }
        thread::sleep(Duration::from_millis(10));
    }

    panic!(
        "timed out waiting for plan prompt; stdout:\n{}",
        String::from_utf8_lossy(stdout)
    );
}

fn read_available_pty_output(master: &mut File, stdout: &mut Vec<u8>) {
    let mut chunk = [0; 8192];
    loop {
        match master.read(&mut chunk) {
            Ok(0) => break,
            Ok(bytes) => stdout.extend_from_slice(&chunk[..bytes]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) if error.raw_os_error() == Some(5) => break,
            Err(error) => panic!("read pty stdout: {error}"),
        }
    }
}

fn set_nonblocking(file: &File) {
    use std::ffi::c_int;

    const F_GETFL: c_int = 3;
    const F_SETFL: c_int = 4;
    #[cfg(target_os = "linux")]
    const O_NONBLOCK: c_int = 0o4000;
    #[cfg(target_os = "macos")]
    const O_NONBLOCK: c_int = 0x0004;

    unsafe extern "C" {
        fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    }

    let flags = unsafe { fcntl(file.as_raw_fd(), F_GETFL) };
    assert_ne!(
        flags,
        -1,
        "fcntl F_GETFL failed: {}",
        std::io::Error::last_os_error()
    );
    let result = unsafe { fcntl(file.as_raw_fd(), F_SETFL, flags | O_NONBLOCK) };
    assert_ne!(
        result,
        -1,
        "fcntl F_SETFL failed: {}",
        std::io::Error::last_os_error()
    );
}
