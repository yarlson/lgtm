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

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr
    );
    assert!(output.stdout.contains(">_ lgtm"));
    assert!(output.stdout.contains("mode:        plan"));
    assert!(output.stdout.contains("permissions: YOLO mode"));
    assert!(output.stdout.contains("Pick one"));
    assert!(output.stdout.contains("Option A"));
    assert!(output.stdout.contains("> /quit"));
    assert_eq!(
        fs::read_to_string(temp.path().join("counter")).expect("counter"),
        "1\n"
    );
    assert!(repo.join(".codex-log/test-plan-001.jsonl").is_file());
    assert!(!temp.path().join("turn-2.json").exists());
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

fn executable(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("codex");
    fs::write(&path, body).expect("script");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod");
    path
}
