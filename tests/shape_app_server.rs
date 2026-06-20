use std::{env, ffi::OsString, fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

#[test]
fn shape_brief_intake_does_not_require_tty() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    let fake_codex = fake_codex_app_server(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("shape")
        .arg("brief idea")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("lgtm shape loop completion is not implemented yet"),
        "{stderr}"
    );
    assert!(!stderr.contains("requires interactive stdin and stdout"));
}

#[test]
fn shape_file_brief_intake_does_not_require_tty() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    fs::write(repo.join("brief.md"), "brief idea").expect("brief");
    let fake_codex = fake_codex_app_server(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("shape")
        .arg("--brief-file")
        .arg("brief.md")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("lgtm shape loop completion is not implemented yet"),
        "{stderr}"
    );
    assert!(!stderr.contains("requires interactive stdin and stdout"));
}

#[test]
fn shape_runtime_preflight_starts_two_sessions_installs_skills_and_logs() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    let fake_codex = fake_codex_app_server(temp.path());
    let _fake_rtk = executable_named(
        temp.path(),
        "rtk",
        "#!/usr/bin/env sh\nprintf '%s\\n' rtk-test\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("shape")
        .arg("brief idea")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test")
        .env("PATH", prepend_path(temp.path()))
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("lgtm shape loop completion is not implemented yet"),
        "{stderr}"
    );
    assert!(stdout.contains(">_ lgtm"));
    assert!(stdout.contains("mode:        shape"));
    assert_eq!(
        stdout
            .matches("Started 2 Codex sessions; gathering context")
            .count(),
        1,
        "{stdout}"
    );
    assert!(stdout.contains("• Ran cargo test"), "{stdout}");
    assert!(stdout.contains("• Edited src/lib.rs"), "{stdout}");
    assert!(stdout.contains("• Searched rust shape output"), "{stdout}");
    assert!(stdout.contains("• Codex"), "{stdout}");
    assert!(stdout.contains("A SPARRING QUESTION"), "{stdout}");
    assert!(!stdout.contains("B HIDDEN DISCOVERY"), "{stdout}");

    assert!(repo.join(".git").is_dir());
    assert!(
        repo.join(".agents/skills/lgtm-phase-implement/SKILL.md")
            .is_file()
    );
    assert!(
        repo.join(".agents/skills/lgtm-security-review/SKILL.md")
            .is_file()
    );
    assert!(
        repo.join(".agents/skills/lgtm-plan-shape/SKILL.md")
            .is_file()
    );
    assert!(repo.join(".lgtm/logs/test-shape-a-001.jsonl").is_file());
    assert!(repo.join(".lgtm/logs/test-shape-b-001.jsonl").is_file());
    assert!(temp.path().join("stopped-1").is_file());
    assert!(temp.path().join("stopped-2").is_file());
    let log_a =
        fs::read_to_string(repo.join(".lgtm/logs/test-shape-a-001.jsonl")).expect("shape a log");
    let log_b =
        fs::read_to_string(repo.join(".lgtm/logs/test-shape-b-001.jsonl")).expect("shape b log");
    assert!(log_a.contains(r#""method\":\"thread/start\""#));
    assert!(log_b.contains(r#""method\":\"thread/start\""#));
    assert!(log_a.contains(r#""method\":\"turn/start\""#));
    assert!(log_b.contains(r#""method\":\"turn/start\""#));
    assert!(log_a.contains("Session role: A"));
    assert!(log_b.contains("Session role: B"));
    let turn_order = fs::read_to_string(temp.path().join("turn-order")).expect("turn order");
    assert_eq!(turn_order.lines().collect::<Vec<_>>(), ["2", "1"]);

    let session_count = fs::read_to_string(temp.path().join("session-counter")).expect("sessions");
    assert_eq!(session_count.trim(), "2");
    let thread_starts =
        fs::read_to_string(temp.path().join("thread-starts.jsonl")).expect("thread starts");
    assert_eq!(thread_starts.lines().count(), 2);
    assert!(thread_starts.contains("RTK - Rust Token Killer"));
}

#[test]
fn shape_raw_mode_echoes_protocol_without_pretty_ui() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    let fake_codex = fake_codex_app_server(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("shape")
        .arg("brief idea")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test")
        .arg("--stream-mode")
        .arg("raw")
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(r#""direction":"out""#), "{stdout}");
    assert!(stdout.contains(r#""method\":\"thread/start\""#), "{stdout}");
    assert!(stdout.contains(r#""method\":\"turn/start\""#), "{stdout}");
    assert!(!stdout.contains(">_ lgtm"), "{stdout}");
    assert!(
        !stdout.contains("Started 2 Codex sessions; gathering context"),
        "{stdout}"
    );
    assert!(repo.join(".lgtm/logs/test-shape-a-001.jsonl").is_file());
    assert!(repo.join(".lgtm/logs/test-shape-b-001.jsonl").is_file());
}

#[test]
fn shape_stops_first_session_when_second_thread_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    let fake_codex = failing_second_thread_codex(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("shape")
        .arg("brief idea")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test")
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to start shape session B thread"),
        "{stderr}"
    );
    assert!(temp.path().join("stopped-1").is_file());
    assert!(temp.path().join("stopped-2").is_file());
}

#[test]
fn shape_reports_role_and_round_when_sparring_turn_fails() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    init_git_repo(&repo);
    let fake_codex = failing_sparring_turn_codex(temp.path());

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("shape")
        .arg("brief idea")
        .arg("--root")
        .arg(&repo)
        .arg("--codex-bin")
        .arg(&fake_codex)
        .arg("--run-stamp")
        .arg("test")
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("shape session A round 1 sparring turn failed"),
        "{stderr}"
    );
    assert!(stderr.contains("sparring boom"), "{stderr}");
    assert!(temp.path().join("stopped-1").is_file());
    assert!(temp.path().join("stopped-2").is_file());
}

fn fake_codex_app_server(dir: &Path) -> std::path::PathBuf {
    executable_named(
        dir,
        "codex",
        r###"#!/usr/bin/env sh
	set -eu
	dir=$(dirname "$0")
	session_counter="$dir/session-counter"
	if [ -f "$session_counter" ]; then
	  session_n=$(cat "$session_counter")
	else
	  session_n=0
	fi
	session_n=$((session_n + 1))
	printf '%s\n' "$session_n" >"$session_counter"

	read initialize
	printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
	read initialized
	read thread_start
	printf '%s\n' "$thread_start" >>"$dir/thread-starts.jsonl"
	printf '{"id":2,"result":{"thread":{"id":"thr-%s"}}}\n' "$session_n"
	turn_n=0
	while IFS= read -r turn_start; do
	  turn_n=$((turn_n + 1))
	  printf '%s\n' "$session_n" >>"$dir/turn-order"
	  printf '%s\n' "$turn_start" >"$dir/turn-$session_n-$turn_n.json"
	  id=$(printf '%s\n' "$turn_start" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
	  turn_id="turn-$session_n-$turn_n"
	  printf '{"id":%s,"result":{"turn":{"id":"%s","status":"inProgress","items":[]}}}\n' "$id" "$turn_id"
	  if [ "$session_n" = 1 ]; then
	    text='A SPARRING QUESTION: 1. Keep current shape 2. Split shape workflow'
	    printf '{"method":"turn/completed","params":{"threadId":"thr-%s","turn":{"id":"%s","status":"completed","items":[{"type":"commandExecution","id":"cmd-%s-%s","command":"cargo test","status":"completed","exitCode":0},{"type":"fileChange","id":"file-%s-%s","status":"completed","changes":[{"kind":"update","path":"src/lib.rs"}]},{"type":"webSearch","id":"web-%s-%s","query":"rust shape output","status":"completed"},{"type":"agentMessage","id":"msg-%s-%s","text":"%s","status":"completed"}]}}}\n' "$session_n" "$turn_id" "$session_n" "$turn_n" "$session_n" "$turn_n" "$session_n" "$turn_n" "$session_n" "$turn_n" "$text"
	  else
	    text='B HIDDEN DISCOVERY'
	    printf '{"method":"turn/completed","params":{"threadId":"thr-%s","turn":{"id":"%s","status":"completed","items":[{"type":"agentMessage","id":"msg-%s-%s","text":"%s","status":"completed"}]}}}\n' "$session_n" "$turn_id" "$session_n" "$turn_n" "$text"
	  fi
	done
	: >"$dir/stopped-$session_n"
	"###,
    )
}

fn failing_sparring_turn_codex(dir: &Path) -> std::path::PathBuf {
    executable_named(
        dir,
        "codex-fails-sparring-turn",
        r###"#!/usr/bin/env sh
	set -eu
	dir=$(dirname "$0")
	session_counter="$dir/session-counter"
	if [ -f "$session_counter" ]; then
	  session_n=$(cat "$session_counter")
	else
	  session_n=0
	fi
	session_n=$((session_n + 1))
	printf '%s\n' "$session_n" >"$session_counter"

	read initialize
	printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
	read initialized
	read thread_start
	printf '{"id":2,"result":{"thread":{"id":"thr-%s"}}}\n' "$session_n"
	turn_n=0
	while IFS= read -r turn_start; do
	  turn_n=$((turn_n + 1))
	  id=$(printf '%s\n' "$turn_start" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
	  turn_id="turn-$session_n-$turn_n"
	  printf '{"id":%s,"result":{"turn":{"id":"%s","status":"inProgress","items":[]}}}\n' "$id" "$turn_id"
	  if [ "$session_n" = 1 ]; then
	    printf '{"method":"turn/completed","params":{"threadId":"thr-%s","turn":{"id":"%s","status":"failed","error":{"message":"sparring boom"},"items":[]}}}\n' "$session_n" "$turn_id"
	  else
	    printf '{"method":"turn/completed","params":{"threadId":"thr-%s","turn":{"id":"%s","status":"completed","items":[{"type":"agentMessage","id":"msg-%s-%s","text":"B HIDDEN DISCOVERY","status":"completed"}]}}}\n' "$session_n" "$turn_id" "$session_n" "$turn_n"
	  fi
	done
	: >"$dir/stopped-$session_n"
	"###,
    )
}

fn failing_second_thread_codex(dir: &Path) -> std::path::PathBuf {
    executable_named(
        dir,
        "codex-fails-second-thread",
        r###"#!/usr/bin/env sh
	set -eu
	dir=$(dirname "$0")
	session_counter="$dir/session-counter"
	if [ -f "$session_counter" ]; then
	  session_n=$(cat "$session_counter")
	else
	  session_n=0
	fi
	session_n=$((session_n + 1))
	printf '%s\n' "$session_n" >"$session_counter"

	read initialize
	printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
	read initialized
	read thread_start
	if [ "$session_n" = 2 ]; then
	  printf '%s\n' '{"id":2,"error":{"code":-1,"message":"boom"}}'
	else
	  printf '%s\n' '{"id":2,"result":{"thread":{"id":"thr-test"}}}'
	fi
	while IFS= read -r _; do
	  :
	done
	: >"$dir/stopped-$session_n"
	"###,
    )
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
