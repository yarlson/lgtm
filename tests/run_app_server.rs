use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

#[test]
fn run_reloads_plan_index_and_runs_three_passes_through_app_server() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    fs::write(
        repo.join("PLAN.md"),
        "# Plan\n\n## Phase 1 - Skeleton\n\nGoal: test.\n\n## Phase 2 - Follow Up\n\nGoal: stale.\n",
    )
    .expect("plan");
    fs::write(repo.join("AGENTS.md"), "# Agents\n").expect("agents");

    let repo_sh = shell_quote(&repo);
    let fake_codex = executable(
        temp.path(),
        &r###"#!/usr/bin/env sh
	set -eu
	repo=__REPO__
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
printf '%s\n' '{"id":2,"result":{"thread":{"id":"thr-test"}}}'
read turn_start
	printf '%s\n' "$turn_start" >"$dir/turn-$n.json"
	printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-test","status":"inProgress","items":[]}}}'
	if [ "$n" = 1 ]; then
	  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","items":[{"type":"agentMessage","id":"msg-index","text":"{\"phases\":[{\"id\":1,\"title\":\"Skeleton\",\"heading\":\"## Phase 1 - Skeleton\"},{\"id\":2,\"title\":\"Follow Up\",\"heading\":\"## Phase 2 - Follow Up\"}]}","status":"completed"}]}}}'
	elif [ "$n" = 2 ]; then
	  cat >"$repo/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Skeleton

Done.

## Phase 2 - Updated Title

Goal: updated.
PLAN
	  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","items":[{"type":"agentMessage","id":"msg-pass","text":"done","status":"completed"}]}}}'
	elif [ "$n" = 5 ]; then
	  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","items":[{"type":"agentMessage","id":"msg-index","text":"{\"phases\":[{\"id\":1,\"title\":\"Skeleton\",\"heading\":\"## Phase 1 - Skeleton\"},{\"id\":2,\"title\":\"Updated Title\",\"heading\":\"## Phase 2 - Updated Title\"}]}","status":"completed"}]}}}'
	else
	  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","items":[{"type":"agentMessage","id":"msg-pass","text":"done","status":"completed"}]}}}'
	fi
	"###
        .replace("__REPO__", &repo_sh),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm-rs"))
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
        .output()
        .expect("run lgtm-rs");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("• Phase 01 implementation: Skeleton"));
    assert!(stdout.contains("• Phase 01 validation: Skeleton"));
    assert!(stdout.contains("• Phase 01 review: Skeleton"));
    assert!(stdout.contains("• Phase 02 implementation: Updated Title"));
    assert!(stdout.contains("• Phase 02 validation: Updated Title"));
    assert!(stdout.contains("• Phase 02 review: Updated Title"));
    assert!(stdout.contains("• Codex"));
    assert!(stdout.contains("  done"));

    assert!(repo.join(".codex-log/test-phase-01-index.jsonl").is_file());
    assert!(
        repo.join(".codex-log/test-phase-01-implement.jsonl")
            .is_file()
    );
    assert!(
        repo.join(".codex-log/test-phase-01-validate.jsonl")
            .is_file()
    );
    assert!(repo.join(".codex-log/test-phase-01-review.jsonl").is_file());
    assert!(repo.join(".codex-log/test-phase-02-index.jsonl").is_file());
    assert!(
        repo.join(".codex-log/test-phase-02-implement.jsonl")
            .is_file()
    );
    let index_log =
        fs::read_to_string(repo.join(".codex-log/test-phase-01-index.jsonl")).expect("index log");
    assert!(index_log.contains(r#""direction":"out""#));
    assert!(index_log.contains(r#""method\":\"turn/start\""#));
    assert!(index_log.contains(r#""direction":"in""#));
    assert!(
        repo.join(".agents/skills/lgtm-phase-implement/SKILL.md")
            .is_file()
    );

    let index_turn = fs::read_to_string(temp.path().join("turn-1.json")).expect("index prompt");
    let implement_turn =
        fs::read_to_string(temp.path().join("turn-2.json")).expect("implement prompt");
    let validate_turn =
        fs::read_to_string(temp.path().join("turn-3.json")).expect("validate prompt");
    let review_turn = fs::read_to_string(temp.path().join("turn-4.json")).expect("review prompt");
    let phase_two_index_turn =
        fs::read_to_string(temp.path().join("turn-5.json")).expect("phase two index prompt");
    let phase_two_implement_turn =
        fs::read_to_string(temp.path().join("turn-6.json")).expect("phase two implement prompt");
    assert!(index_turn.contains("gpt-5.4-mini") || index_turn.contains("PLAN.md content"));
    assert!(index_turn.contains("## Phase 2 - Follow Up"));
    assert!(implement_turn.contains("$lgtm-phase-implement"));
    assert!(implement_turn.contains("## Phase 1 - Skeleton"));
    assert!(validate_turn.contains("$lgtm-phase-validate"));
    assert!(review_turn.contains("$lgtm-phase-review"));
    assert!(phase_two_index_turn.contains("## Phase 2 - Updated Title"));
    assert!(phase_two_implement_turn.contains("## Phase 2 - Updated Title"));
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

fn executable(dir: &Path, body: &str) -> std::path::PathBuf {
    let path = dir.join("codex");
    fs::write(&path, body).expect("script");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod");
    path
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}
