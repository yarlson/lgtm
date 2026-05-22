use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

use chrono::Local;

use crate::Error;
use crate::cli::Config;
use crate::cli::StreamMode;
use crate::events::CodexEvent;
use crate::plan;
use crate::prompt;
use crate::render::Renderer;
use crate::skills;

pub fn run_plan(config: Config) -> Result<(), Error> {
    let renderer = Renderer::new();

    plan::require_file(&config.plan_abs(), &config.plan_path)?;
    plan::require_file(&config.agents_abs(), &config.agents_path)?;
    plan::require_file(&config.design_abs(), &config.design_path)?;
    skills::install(&config.root)?;

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

    for phase in config.start_phase..=end_phase {
        let title = plan::phase_title(&plan_text, phase).ok_or_else(|| {
            Error::message(format!(
                "Phase {phase} was not found in {}",
                config.plan_path.display()
            ))
        })?;

        run_phase_prompt(
            &config,
            &renderer,
            phase,
            &title,
            "implement",
            prompt::implementation_prompt(
                &config.plan_path,
                &config.agents_path,
                &config.design_path,
                phase,
                &title,
            ),
        )?;

        run_phase_prompt(
            &config,
            &renderer,
            phase,
            &title,
            "validate",
            prompt::validation_prompt(
                &config.plan_path,
                &config.agents_path,
                &config.design_path,
                phase,
                &title,
            ),
        )?;

        if phase < end_phase {
            renderer.sleep(config.sleep_seconds, phase + 1);
            thread::sleep(Duration::from_secs(config.sleep_seconds));
        }
    }

    Ok(())
}

fn run_phase_prompt(
    config: &Config,
    renderer: &Renderer,
    phase: u32,
    title: &str,
    action: &str,
    prompt: String,
) -> Result<(), Error> {
    let log_name = format!("{}-phase-{phase:02}-{action}.jsonl", config.run_stamp,);
    let log_path = config.log_dir.join(log_name);
    let log_display = log_path.display().to_string();

    renderer.phase_header(phase, title, action, &log_display);
    renderer.system(format!(
        "[{}] writing raw JSONL to {}",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        log_path.display()
    ));

    fs::create_dir_all(&config.log_dir).map_err(|source| Error::io(&config.log_dir, source))?;
    let mut log = File::create(&log_path).map_err(|source| Error::io(&log_path, source))?;

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
        .ok_or_else(|| Error::message("failed to open codex stdin"))?;
    let writer = thread::spawn(move || -> std::io::Result<()> {
        stdin.write_all(prompt.as_bytes())?;
        stdin.write_all(b"\n")?;
        Ok(())
    });

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::message("failed to open codex stdout"))?;
    let reader = BufReader::new(stdout);
    for line in reader.split(b'\n') {
        let mut line = line.map_err(|source| Error::io(&log_path, source))?;
        if line.is_empty() {
            continue;
        }
        line.push(b'\n');
        log.write_all(&line)
            .map_err(|source| Error::io(&log_path, source))?;

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

    writer
        .join()
        .map_err(|_| Error::message("codex stdin writer panicked"))?
        .map_err(|source| Error::io("<codex stdin>", source))?;

    let status = child
        .wait()
        .map_err(|source| Error::io(&config.codex_bin, source))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::CodexStatus { status })
    }
}
