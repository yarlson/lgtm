use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Child;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::Error;
use crate::cli::Config;
use crate::cli::StreamMode;
use crate::events::CodexEvent;
use crate::git;
use crate::plan;
use crate::prompt;
use crate::render::Renderer;
use crate::skills;

pub fn run_plan(config: Config) -> Result<(), Error> {
    let mut renderer = Renderer::new();

    plan::require_file(&config.plan_abs(), &config.plan_path)?;
    plan::require_file(&config.agents_abs(), &config.agents_path)?;
    plan::require_file(&config.design_abs(), &config.design_path)?;
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
                prompt::phase_prompt(
                    &config.plan_path,
                    &config.agents_path,
                    &config.design_path,
                    &phase,
                    pass,
                ),
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
    let log_display = log_path.display().to_string();

    renderer.phase_header(phase.number, &phase.title, action, &log_display);
    renderer.system(format!("raw_jsonl {}", log_path.display()));

    fs::create_dir_all(&config.log_dir).map_err(|source| Error::io(&config.log_dir, source))?;
    let mut log = File::create(&log_path).map_err(|source| Error::io(&log_path, source))?;

    let (process, stdout) = CodexProcess::spawn(config, prompt)?;
    let stream_result = stream_codex_output(config, renderer, &log_path, &mut log, stdout);
    process.finish(&config.codex_bin, stream_result)
}

struct CodexProcess {
    child: Child,
    stdin_writer: JoinHandle<std::io::Result<()>>,
}

impl CodexProcess {
    fn spawn(config: &Config, prompt: String) -> Result<(Self, ChildStdout), Error> {
        let mut child = Command::new(&config.codex_bin)
            .arg("exec")
            .arg("-C")
            .arg(&config.root)
            .arg("--dangerously-bypass-approvals-and-sandbox")
            .arg("--json")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|source| Error::io(&config.codex_bin, source))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| finish_spawn_error(&mut child, "failed to open codex stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| finish_spawn_error(&mut child, "failed to open codex stdout"))?;

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
                renderer.tick();
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

        match config.stream_mode {
            StreamMode::Raw => {
                std::io::stdout()
                    .write_all(&line)
                    .map_err(|source| Error::io("<stdout>", source))?;
            }
            StreamMode::Pretty => {
                let line_str = String::from_utf8_lossy(&line);
                match CodexEvent::parse(&line_str) {
                    Ok(event) => renderer.event(&event),
                    Err(error) => renderer.raw_parse_error(&line_str, &error),
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn run_plan_rejects_unmanaged_snap_skill_before_git_init() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        fs::write(root.join("PLAN.md"), "# Plan\n\n## Phase 1: Test\n").expect("plan");
        fs::write(root.join("AGENTS.md"), "# Agents\n").expect("agents");
        fs::write(root.join("DESIGN.md"), "# Design\n").expect("design");

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
            design_path: "DESIGN.md".into(),
            start_phase: 1,
            end_phase: Some(1),
            sleep_seconds: 0,
            codex_bin: "codex".to_string(),
            stream_mode: StreamMode::Pretty,
            log_dir: root.join(".codex-log"),
            run_stamp: "test".to_string(),
        };

        let error = run_plan(config).expect_err("unmanaged skill should abort");

        assert!(error.to_string().contains("is not managed by snap-rs"));
        assert!(!root.join(".git").exists());
    }
}
