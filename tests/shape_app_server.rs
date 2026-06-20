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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(!stderr.contains("requires interactive stdin and stdout"));
    assert!(repo.join("PLAN.md").is_file());
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(!stderr.contains("requires interactive stdin and stdout"));
    assert!(repo.join("PLAN.md").is_file());
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
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
    assert!(stdout.contains("• Updated Plan"), "{stdout}");
    assert!(stdout.contains("Inspect shape workflow"), "{stdout}");
    assert!(stdout.contains("• Edited src/lib.rs"), "{stdout}");
    assert!(stdout.contains("• Searched rust shape output"), "{stdout}");
    assert!(stdout.contains("• Shape 01 sparring"), "{stdout}");
    assert!(stdout.contains("• Shape 02 sparring"), "{stdout}");
    assert!(stdout.contains("• Codex"), "{stdout}");
    assert!(stdout.contains("A SPARRING QUESTION"), "{stdout}");
    assert!(stdout.contains("PLAN_PATH: PLAN.md"), "{stdout}");
    assert!(stdout.contains("Final plan: "), "{stdout}");
    assert!(stdout.contains("PLAN.md"), "{stdout}");
    assert!(stdout.contains("• Tokens: input 40 (cached 32), output 8, reasoning 4, total 48"));
    assert!(!stdout.contains("B HIDDEN DISCOVERY"), "{stdout}");
    assert!(!stdout.contains("2, but keep local UX"), "{stdout}");
    assert!(repo.join("PLAN.md").is_file());

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
    assert_eq!(turn_order.lines().collect::<Vec<_>>(), ["2", "1", "2", "1"]);
    let question_prompt =
        fs::read_to_string(temp.path().join("turn-2-2.json")).expect("question prompt");
    assert!(question_prompt.contains("Session A asked this forced-choice question"));
    assert!(question_prompt.contains("Session A assistant excerpt"));
    assert!(question_prompt.contains("A SPARRING QUESTION"));
    let answer_prompt =
        fs::read_to_string(temp.path().join("turn-1-2.json")).expect("answer prompt");
    assert!(answer_prompt.contains("Session B answered the previous forced-choice question"));
    assert!(answer_prompt.contains("Evidence answer:\\n2, but keep local UX"));
    assert!(answer_prompt.contains("2, but keep local UX"));

    let session_count = fs::read_to_string(temp.path().join("session-counter")).expect("sessions");
    assert_eq!(session_count.trim(), "2");
    let thread_starts =
        fs::read_to_string(temp.path().join("thread-starts.jsonl")).expect("thread starts");
    assert_eq!(thread_starts.lines().count(), 2);
    assert!(thread_starts.contains("RTK - Rust Token Killer"));
}

#[test]
fn shape_rejects_large_invalid_evidence_answer_before_returning_it_to_sparring_session() {
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
        .env("LGTM_TEST_LARGE_EVIDENCE", "1")
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Session B evidence answer remained invalid after one repair attempt"),
        "{stderr}"
    );
    let repair_prompt =
        fs::read_to_string(temp.path().join("turn-2-3.json")).expect("repair prompt");
    assert!(repair_prompt.contains("[truncated to 4000 chars]"));
    assert!(!repair_prompt.contains("TAIL_SHOULD_NOT_REACH_SESSION_A"));
    assert!(!temp.path().join("turn-1-2.json").exists());
}

#[test]
fn shape_repairs_invalid_evidence_answer_once() {
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
        .env("LGTM_TEST_INVALID_EVIDENCE_ONCE", "1")
        .output()
        .expect("run lgtm");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    let repair_prompt =
        fs::read_to_string(temp.path().join("turn-2-3.json")).expect("repair prompt");
    assert!(repair_prompt.contains("Your previous evidence answer did not match"));
    assert!(repair_prompt.contains("Original forced-choice question"));
    assert!(repair_prompt.contains("A SPARRING QUESTION"));
    assert!(repair_prompt.contains("Invalid answer"));
    assert!(repair_prompt.contains("I recommend option 2 because it is cleaner."));
    let answer_prompt =
        fs::read_to_string(temp.path().join("turn-1-2.json")).expect("answer prompt");
    assert!(answer_prompt.contains("Evidence answer:\\n3"));
    assert!(!answer_prompt.contains("I recommend option 2"));
    let turn_order = fs::read_to_string(temp.path().join("turn-order")).expect("turn order");
    assert_eq!(
        turn_order.lines().collect::<Vec<_>>(),
        ["2", "1", "2", "2", "1"]
    );
    assert!(repo.join("PLAN.md").is_file());
}

#[test]
fn shape_exits_after_final_marker_and_plan_file_creation() {
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
        .output()
        .expect("run lgtm");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.contains("PLAN_PATH: PLAN.md"), "{stdout}");
    assert!(stdout.contains("Final plan: "), "{stdout}");
    assert!(repo.join("PLAN.md").is_file());
    let turn_order = fs::read_to_string(temp.path().join("turn-order")).expect("turn order");
    assert_eq!(turn_order.lines().collect::<Vec<_>>(), ["2", "1", "2", "1"]);
}

#[test]
fn shape_fails_when_fake_codex_created_plan_breaks_final_contract() {
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
        .env("LGTM_TEST_INVALID_FINAL_PLAN_CONTRACT", "1")
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing required `Steps:`"), "{stderr}");
    assert!(!stdout.contains("Final plan: "), "{stdout}");
}

#[test]
fn shape_sends_finalization_once_after_max_rounds() {
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
        .arg("--max-rounds")
        .arg("1")
        .env("LGTM_TEST_FINALIZE_AFTER_MAX", "1")
        .output()
        .expect("run lgtm");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(repo.join("PLAN.md").is_file());
    let turn_order = fs::read_to_string(temp.path().join("turn-order")).expect("turn order");
    assert_eq!(turn_order.lines().collect::<Vec<_>>(), ["2", "1", "1"]);
    let finalization_prompt =
        fs::read_to_string(temp.path().join("turn-1-2.json")).expect("finalization prompt");
    assert!(finalization_prompt.contains("The host reached --max-rounds=1"));
    assert!(!temp.path().join("turn-1-3.json").exists());
}

#[test]
fn shape_fails_when_final_marker_plan_file_is_missing() {
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
        .env("LGTM_TEST_MISSING_FINAL_PLAN", "1")
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reported shape plan path does not exist"),
        "{stderr}"
    );
}

#[test]
fn shape_fails_when_finalization_does_not_report_plan_marker() {
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
        .arg("--max-rounds")
        .arg("1")
        .env("LGTM_TEST_FINALIZATION_NO_MARKER", "1")
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("finalization did not report a final plan"),
        "{stderr}"
    );
    assert!(!repo.join("PLAN.md").exists());
    let turn_order = fs::read_to_string(temp.path().join("turn-order")).expect("turn order");
    assert_eq!(turn_order.lines().collect::<Vec<_>>(), ["2", "1", "1"]);
}

#[test]
fn shape_fails_when_repaired_evidence_answer_is_still_invalid() {
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
        .env("LGTM_TEST_INVALID_EVIDENCE_TWICE", "1")
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Session B evidence answer remained invalid after one repair attempt"),
        "{stderr}"
    );
    assert!(stderr.contains("still invalid after repair"), "{stderr}");
    assert!(!temp.path().join("turn-1-2.json").exists());
    assert!(temp.path().join("stopped-1").is_file());
    assert!(temp.path().join("stopped-2").is_file());
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

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
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
	cwd=$(printf '%s\n' "$thread_start" | sed -n 's/.*"cwd":"\([^"]*\)".*/\1/p')
	cd "$cwd"
	printf '%s\n' "$thread_start" >>"$dir/thread-starts.jsonl"
	printf '{"id":2,"result":{"thread":{"id":"thr-%s"}}}\n' "$session_n"
	turn_n=0
	write_plan() {
	  if [ "${LGTM_TEST_INVALID_FINAL_PLAN_CONTRACT:-}" = 1 ]; then
	    printf '# Plan\n\n## Phase 1 - Test\n\nGoal:\nShip.\n\nValidation:\n- Check it.\n' > PLAN.md
	  else
	    printf '# Plan\n\n## Phase 1 - Test\n\nGoal:\nShip.\n\nSteps:\n- Do it.\n\nValidation:\n- Check it.\n' > PLAN.md
	  fi
	}
	emit_plan_update() {
	  printf '{"method":"turn/plan/updated","params":{"threadId":"thr-%s","turnId":"%s","plan":[{"step":"Inspect shape workflow","status":"completed"},{"step":"Write final PLAN.md","status":"inProgress"}]}}\n' "$session_n" "$turn_id"
	}
	while IFS= read -r turn_start; do
	  turn_n=$((turn_n + 1))
	  printf '%s\n' "$session_n" >>"$dir/turn-order"
	  printf '%s\n' "$turn_start" >"$dir/turn-$session_n-$turn_n.json"
	  id=$(printf '%s\n' "$turn_start" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
	  turn_id="turn-$session_n-$turn_n"
	  printf '{"id":%s,"result":{"turn":{"id":"%s","status":"inProgress","items":[]}}}\n' "$id" "$turn_id"
	  if [ "$session_n" = 1 ]; then
	    emit_plan_update
	    if [ "$turn_n" = 1 ]; then
	      text='A SPARRING QUESTION: 1. Keep current shape 2. Split shape workflow'
	    elif [ "${LGTM_TEST_FINALIZATION_NO_MARKER:-}" = 1 ]; then
	      text='BLOCKER: no plan could be produced'
	    elif [ "${LGTM_TEST_FINALIZE_AFTER_MAX:-}" = 1 ]; then
	      write_plan
	      text='PLAN_PATH: PLAN.md'
	    else
	      if [ "${LGTM_TEST_MISSING_FINAL_PLAN:-}" != 1 ]; then
	        write_plan
	      fi
	      text='PLAN_PATH: PLAN.md'
	    fi
	    printf '{"method":"turn/completed","params":{"threadId":"thr-%s","turn":{"id":"%s","status":"completed","usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":8},"output_tokens":2,"output_tokens_details":{"reasoning_tokens":1},"total_tokens":12},"items":[{"type":"commandExecution","id":"cmd-%s-%s","command":"cargo test","status":"completed","exitCode":0},{"type":"fileChange","id":"file-%s-%s","status":"completed","changes":[{"kind":"update","path":"src/lib.rs"}]},{"type":"webSearch","id":"web-%s-%s","query":"rust shape output","status":"completed"},{"type":"agentMessage","id":"msg-%s-%s","text":"%s","status":"completed"}]}}}\n' "$session_n" "$turn_id" "$session_n" "$turn_n" "$session_n" "$turn_n" "$session_n" "$turn_n" "$session_n" "$turn_n" "$text"
	  elif [ "$turn_n" = 1 ]; then
	    text='B HIDDEN DISCOVERY'
	    printf '{"method":"turn/completed","params":{"threadId":"thr-%s","turn":{"id":"%s","status":"completed","usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":8},"output_tokens":2,"output_tokens_details":{"reasoning_tokens":1},"total_tokens":12},"items":[{"type":"agentMessage","id":"msg-%s-%s","text":"%s","status":"completed"}]}}}\n' "$session_n" "$turn_id" "$session_n" "$turn_n" "$text"
	  else
	    if [ "${LGTM_TEST_LARGE_EVIDENCE:-}" = 1 ]; then
	      text=$(printf 'x%.0s' $(seq 1 4500))
	      text="${text}TAIL_SHOULD_NOT_REACH_SESSION_A"
	    elif [ "${LGTM_TEST_INVALID_EVIDENCE_TWICE:-}" = 1 ]; then
	      if [ "$turn_n" = 2 ]; then
	        text='I recommend option 2 because it is cleaner.'
	      else
	        text='still invalid after repair'
	      fi
	    elif [ "${LGTM_TEST_INVALID_EVIDENCE_ONCE:-}" = 1 ]; then
	      if [ "$turn_n" = 2 ]; then
	        text='I recommend option 2 because it is cleaner.'
	      else
	        text='3'
	      fi
	    else
	      text='2, but keep local UX'
	    fi
	    printf '{"method":"turn/completed","params":{"threadId":"thr-%s","turn":{"id":"%s","status":"completed","usage":{"input_tokens":10,"input_tokens_details":{"cached_tokens":8},"output_tokens":2,"output_tokens_details":{"reasoning_tokens":1},"total_tokens":12},"items":[{"type":"agentMessage","id":"msg-%s-%s","text":"%s","status":"completed"}]}}}\n' "$session_n" "$turn_id" "$session_n" "$turn_n" "$text"
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
