use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

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
    fs::write(repo.join("DESIGN.md"), "# Design\n").expect("write design");

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
    let mut perms = fs::metadata(&fake_codex)
        .expect("fake metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&fake_codex, perms).expect("chmod fake codex");

    let output = Command::new(env!("CARGO_BIN_EXE_snap-rs"))
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
        .expect("run snap-rs");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("phase=01"));
    assert!(stdout.contains("pass=review"));
    assert!(stdout.contains("thread"));
    assert!(stdout.contains("codex"));
    assert!(stdout.contains("turn"));

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
    assert!(implement_prompt.contains("$snap-phase-implement"));
    assert!(validate_prompt.contains("$snap-phase-validate"));
    assert!(review_prompt.contains("$snap-phase-review"));

    let default_logs = repo.join(".codex-log");
    assert!(default_logs.join("test-phase-01-implement.jsonl").is_file());
    assert!(default_logs.join("test-phase-01-validate.jsonl").is_file());
    assert!(default_logs.join("test-phase-01-review.jsonl").is_file());
    assert!(
        repo.join(".agents")
            .join("skills")
            .join("snap-phase-implement")
            .join("SKILL.md")
            .is_file()
    );
    assert!(
        repo.join(".agents")
            .join("skills")
            .join("snap-phase-review")
            .join("SKILL.md")
            .is_file()
    );
    assert!(
        fs::read_to_string(repo.join(".gitignore"))
            .expect("read gitignore")
            .lines()
            .any(|line| line == ".agents/skills/snap-*")
    );
    assert!(
        fs::read_to_string(repo.join(".gitignore"))
            .expect("read gitignore")
            .lines()
            .any(|line| line == ".codex-log/")
    );
}
