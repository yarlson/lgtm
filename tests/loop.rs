use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn runs_implementation_and_validation_prompts_with_formatted_output() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let repo = temp.path().join("repo");
    let logs = temp.path().join("logs");
    fs::create_dir(&repo).expect("create repo");
    fs::write(
        repo.join("PLAN.md"),
        "# Plan\n\n## Phase 1 - Skeleton\n\nGoal: test.\n",
    )
    .expect("write plan");
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("write agents");
    fs::write(repo.join("DESIGN.md"), "# Design\n").expect("write design");

    let fake_codex = temp.path().join("codex");
    fs::write(
        &fake_codex,
        r#"#!/usr/bin/env sh
set -eu
cat >/dev/null
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
        .arg("--log-dir")
        .arg(&logs)
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
    assert!(stdout.contains("Phase 01"));
    assert!(stdout.contains("thread"));
    assert!(stdout.contains("codex"));
    assert!(stdout.contains("turn"));

    assert!(logs.join("test-phase-01-implement.jsonl").is_file());
    assert!(logs.join("test-phase-01-validate.jsonl").is_file());
}
