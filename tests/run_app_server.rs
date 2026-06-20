use std::{env, ffi::OsString, fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

#[test]
fn run_reloads_plan_index_and_runs_four_passes_through_app_server() {
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
    let codex_source_home = temp.path().join("codex-source");
    fs::create_dir(&codex_source_home).expect("codex source home");
    fs::write(codex_source_home.join("auth.json"), "auth").expect("auth");
    fs::write(codex_source_home.join("config.toml"), "config").expect("config");
    fs::create_dir(codex_source_home.join("skills")).expect("skills dir");
    fs::write(codex_source_home.join("skills").join("stale"), "stale").expect("stale");

    let repo_sh = shell_quote(&repo);
    let fake_codex = executable(
        temp.path(),
        &r###"#!/usr/bin/env sh
	set -eu
	repo=__REPO__
	dir=$(dirname "$0")
	session_counter="$dir/session-counter"
	if [ -f "$session_counter" ]; then
	  session_n=$(cat "$session_counter")
	else
	  session_n=0
	fi
	session_n=$((session_n + 1))
	printf '%s\n' "$session_n" >"$session_counter"
	printf '%s\n' "${CODEX_HOME:-}" >"$dir/codex-home-$session_n"

	read initialize
	printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
	read initialized
	read thread_start
	printf '%s\n' "$thread_start" >>"$dir/thread-starts.jsonl"
	printf '%s\n' '{"id":2,"result":{"thread":{"id":"thr-test"}}}'
	while IFS= read -r turn_start; do
	  turn_counter="$dir/turn-counter"
	  if [ -f "$turn_counter" ]; then
	    turn_n=$(cat "$turn_counter")
	  else
	    turn_n=0
	  fi
	  turn_n=$((turn_n + 1))
	  printf '%s\n' "$turn_n" >"$turn_counter"
	  id=$(printf '%s\n' "$turn_start" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
	  printf '%s\n' "$turn_start" >"$dir/turn-$turn_n.json"
	  printf '{"id":%s,"result":{"turn":{"id":"turn-test","status":"inProgress","items":[]}}}\n' "$id"
		if [ "$turn_n" = 1 ]; then
		  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","usage":{"input_tokens":100,"input_tokens_details":{"cached_tokens":80},"output_tokens":20,"output_tokens_details":{"reasoning_tokens":5},"total_tokens":120},"items":[{"type":"agentMessage","id":"msg-index","text":"{\"phases\":[{\"id\":1,\"title\":\"Skeleton\",\"heading\":\"## Phase 1 - Skeleton\"},{\"id\":2,\"title\":\"Follow Up\",\"heading\":\"## Phase 2 - Follow Up\"}]}","status":"completed"}]}}}'
		elif [ "$turn_n" = 2 ]; then
		  cat >"$repo/PLAN.md" <<-'PLAN'
	# Plan

## Phase 1 - Skeleton

Done.

## Phase 2 - Updated Title

Goal: updated.
	PLAN
		  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":8},"output_tokens":2,"output_tokens_details":{"reasoning_tokens":1},"total_tokens":12},"items":[{"type":"agentMessage","id":"msg-pass","text":"done","status":"completed"}]}}}'
		elif [ "$turn_n" = 6 ]; then
		  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","items":[{"type":"agentMessage","id":"msg-index","text":"{\"phases\":[{\"id\":1,\"title\":\"Skeleton\",\"heading\":\"## Phase 1 - Skeleton\"},{\"id\":2,\"title\":\"Updated Title\",\"heading\":\"## Phase 2 - Updated Title\"}]}","status":"completed"}]}}}'
		elif [ "$turn_n" = 7 ]; then
		  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","usage":{"input_tokens":30,"input_tokens_details":{"cached_tokens":24},"output_tokens":6,"output_tokens_details":{"reasoning_tokens":3},"total_tokens":36},"items":[{"type":"agentMessage","id":"msg-pass","text":"done","status":"completed"}]}}}'
		else
		  printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-test","turn":{"id":"turn-test","status":"completed","items":[{"type":"agentMessage","id":"msg-pass","text":"done","status":"completed"}]}}}'
		fi
	done
		"###
        .replace("__REPO__", &repo_sh),
    );
    let _fake_rtk = executable_named(
        temp.path(),
        "rtk",
        "#!/usr/bin/env sh\nprintf '%s\\n' rtk-test\n",
    );
    let path_with_rtk = prepend_path(temp.path());

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
        .env("CODEX_HOME", &codex_source_home)
        .env("PATH", path_with_rtk)
        .output()
        .expect("run lgtm");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(">_ lgtm"));
    assert!(stdout.contains("mode:        run"));
    assert!(stdout.contains("execution:   host YOLO"));
    assert!(stdout.contains("• Phase 01 implementation: Skeleton"));
    assert!(stdout.contains("• Phase 01 validation: Skeleton"));
    assert!(stdout.contains("• Phase 01 review: Skeleton"));
    assert!(stdout.contains("• Phase 01 commit: Skeleton"));
    assert!(stdout.contains("• Phase 02 implementation: Updated Title"));
    assert!(stdout.contains("• Phase 02 validation: Updated Title"));
    assert!(stdout.contains("• Phase 02 review: Updated Title"));
    assert!(stdout.contains("• Phase 02 commit: Updated Title"));
    assert!(stdout.contains("• Codex"));
    assert!(stdout.contains("  done"));
    assert!(
        stdout
            .contains("• Phase 1 tokens: input 110 (cached 88), output 22, reasoning 6, total 132")
    );
    assert!(
        stdout.contains("• Phase 2 tokens: input 30 (cached 24), output 6, reasoning 3, total 36")
    );
    assert!(stdout.contains("• Tokens: input 140 (cached 112), output 28, reasoning 9, total 168"));

    assert!(repo.join(".lgtm/logs/test-phase-01-index.jsonl").is_file());
    assert!(
        repo.join(".lgtm/logs/test-phase-01-implement.jsonl")
            .is_file()
    );
    assert!(
        repo.join(".lgtm/logs/test-phase-01-validate.jsonl")
            .is_file()
    );
    assert!(repo.join(".lgtm/logs/test-phase-01-review.jsonl").is_file());
    assert!(repo.join(".lgtm/logs/test-phase-01-commit.jsonl").is_file());
    assert!(repo.join(".lgtm/logs/test-phase-02-index.jsonl").is_file());
    assert!(
        repo.join(".lgtm/logs/test-phase-02-implement.jsonl")
            .is_file()
    );
    let index_log =
        fs::read_to_string(repo.join(".lgtm/logs/test-phase-01-index.jsonl")).expect("index log");
    assert!(index_log.contains(r#""direction":"out""#));
    assert!(index_log.contains(r#""method\":\"turn/start\""#));
    assert!(index_log.contains(r#""direction":"in""#));
    assert!(
        repo.join(".agents/skills/lgtm-phase-implement/SKILL.md")
            .is_file()
    );
    assert!(
        repo.join(".agents/skills/lgtm-phase-commit/SKILL.md")
            .is_file()
    );
    let thread_starts =
        fs::read_to_string(temp.path().join("thread-starts.jsonl")).expect("thread starts");
    assert!(thread_starts.contains("RTK - Rust Token Killer"));
    assert!(thread_starts.contains("Always prefix shell commands with `rtk`."));

    let index_turn = fs::read_to_string(temp.path().join("turn-1.json")).expect("index prompt");
    let implement_turn =
        fs::read_to_string(temp.path().join("turn-2.json")).expect("implement prompt");
    let validate_turn =
        fs::read_to_string(temp.path().join("turn-3.json")).expect("validate prompt");
    let review_turn = fs::read_to_string(temp.path().join("turn-4.json")).expect("review prompt");
    let phase_two_index_turn =
        fs::read_to_string(temp.path().join("turn-6.json")).expect("phase two index prompt");
    let phase_two_implement_turn =
        fs::read_to_string(temp.path().join("turn-7.json")).expect("phase two implement prompt");
    let commit_turn = fs::read_to_string(temp.path().join("turn-5.json")).expect("commit prompt");
    let session_count = fs::read_to_string(temp.path().join("session-counter")).expect("sessions");
    assert_eq!(session_count.trim(), "4");
    let child_codex_home =
        fs::read_to_string(temp.path().join("codex-home-1")).expect("child codex home");
    let child_codex_home = child_codex_home.trim();
    assert_ne!(child_codex_home, codex_source_home.display().to_string());
    assert!(child_codex_home.contains("lgtm-codex-home-"));
    assert!(index_turn.contains("gpt-5.4-mini") || index_turn.contains("PLAN.md content"));
    assert!(index_turn.contains(r#""effort":"low""#));
    assert!(index_turn.contains("## Phase 2 - Follow Up"));
    assert!(implement_turn.contains("$lgtm-phase-implement"));
    assert!(implement_turn.contains(r#""effort":"high""#));
    assert!(implement_turn.contains("## Phase 1 - Skeleton"));
    assert!(validate_turn.contains("$lgtm-phase-validate"));
    assert!(validate_turn.contains(r#""effort":"medium""#));
    assert!(review_turn.contains("$lgtm-phase-review"));
    assert!(review_turn.contains(r#""effort":"high""#));
    assert!(commit_turn.contains("$lgtm-phase-commit"));
    assert!(commit_turn.contains(r#""effort":"low""#));
    assert!(commit_turn.contains("## Phase 1 - Skeleton"));
    assert!(
        commit_turn.contains("Create a real git commit with a concise Conventional Commit subject")
    );
    assert!(commit_turn.contains("Never include changed-file lists"));
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
    executable_named(dir, "codex", body)
}

fn executable_named(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("script");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod");
    path
}

fn prepend_path(directory: &Path) -> OsString {
    let current_path = env::var_os("PATH").unwrap_or_default();
    env::join_paths(std::iter::once(directory.to_path_buf()).chain(env::split_paths(&current_path)))
        .expect("PATH")
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}
