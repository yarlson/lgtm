use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{
    app_server::{AppServerClient, CompletedTurn, ItemKind, TurnControl},
    cli::PlanArgs,
    commands::execution::ExecutionConfig,
    commands::runtime::{CommandRuntime, CommandRuntimeConfig, RuntimeAppServerClient},
    composer::{self, ComposerSubmission},
    git,
    output::{
        RenderOptions, Renderer,
        banner::{self, Banner, BannerMode},
    },
    prompt, skills,
};

#[derive(Debug, Clone)]
struct PlanConfig {
    runtime: CommandRuntime,
    plan_path: PathBuf,
    agents_path: PathBuf,
    brief: Option<String>,
}

impl PlanConfig {
    fn from_args(args: PlanArgs) -> Result<Self> {
        let runtime = CommandRuntime::new(CommandRuntimeConfig {
            root: args.root,
            log_dir: args.log_dir,
            run_stamp: args.run_stamp,
            execution: ExecutionConfig::from_args(args.codex_bin, args.execution),
        })?;

        Ok(Self {
            runtime,
            plan_path: args.plan_path,
            agents_path: "AGENTS.md".into(),
            brief: args.brief,
        })
    }

    fn plan_abs(&self) -> PathBuf {
        self.runtime.resolve_root_path(&self.plan_path)
    }

    fn agents_abs(&self) -> PathBuf {
        self.runtime.resolve_root_path(&self.agents_path)
    }

    fn turn_log_name(&self, turn_number: u32) -> String {
        format!("{}-plan-{turn_number:03}.jsonl", self.runtime.run_stamp())
    }
}

pub fn run(args: PlanArgs) -> Result<()> {
    require_planning_tty()?;

    let config = PlanConfig::from_args(args)?;
    let mut stdout = io::stdout();
    write_planning_output(
        &mut stdout,
        banner::render(
            Banner {
                mode: BannerMode::Plan,
                root: config.runtime.root(),
                codex_bin: config.runtime.app_server_binary(),
                execution: config.runtime.execution_label(),
            },
            &RenderOptions::default(),
        ),
    )?;

    skills::preflight(config.runtime.root())?;
    git::ensure_initialized(config.runtime.root())?;
    skills::install(config.runtime.root())?;

    let artifacts_before = PlanningArtifactsSnapshot::capture(&config)?;
    let mut client = connect_client(&config)?;
    set_turn_log(&config, &mut client, 1)?;
    let thread_id = client.start_thread()?;
    let first_prompt = prompt::plan_initial_prompt(
        &config.plan_path,
        &config.agents_path,
        config.brief.as_deref(),
    );
    let mut turn_number = 1;
    let mut artifacts_complete = run_planning_turn(
        &config,
        &mut client,
        &thread_id,
        first_prompt,
        &artifacts_before,
        &mut stdout,
    )?;

    loop {
        if artifacts_complete {
            return client.stop();
        }

        let resume_prompt = match composer::read_inline_answer()? {
            ComposerSubmission::Quit => return client.stop(),
            ComposerSubmission::Finish => prompt::plan_resume_prompt("/finish"),
            ComposerSubmission::Answer(answer) => prompt::plan_resume_prompt(&answer),
        };

        turn_number += 1;
        set_turn_log(&config, &mut client, turn_number)?;
        artifacts_complete = run_planning_turn(
            &config,
            &mut client,
            &thread_id,
            resume_prompt,
            &artifacts_before,
            &mut stdout,
        )?;
    }
}

fn run_planning_turn(
    config: &PlanConfig,
    client: &mut AppServerClient,
    thread_id: &str,
    prompt: String,
    artifacts_before: &PlanningArtifactsSnapshot,
    output: &mut impl Write,
) -> Result<bool> {
    let mut renderer = Renderer::new(RenderOptions::default());
    write_planning_output(output, renderer.planning_header())?;
    let mut output_result = Ok(());
    let turn = client.run_turn_streaming(thread_id, &prompt, |event| {
        if output_result.is_ok() {
            output_result = write_planning_output(output, renderer.render_event(&event));
        }
        TurnControl::Continue
    })?;
    if output_result.is_ok() {
        output_result = write_planning_output(output, renderer.finish());
    }
    output_result?;

    let artifacts_after = PlanningArtifactsSnapshot::capture(config)?;
    let has_agent_message = has_agent_message(&turn);
    let artifacts_complete = artifacts_before.is_complete_with(&artifacts_after);
    if !has_agent_message && !artifacts_complete {
        bail!(
            "codex plan turn completed without an agent message and did not complete planning artifacts"
        );
    }

    Ok(artifacts_complete)
}

fn write_planning_output(output: &mut impl Write, rendered: String) -> Result<()> {
    if rendered.is_empty() {
        return Ok(());
    }

    output
        .write_all(rendered.as_bytes())
        .context("failed to write planning output")?;
    output.flush().context("failed to flush planning output")
}

fn has_agent_message(turn: &CompletedTurn) -> bool {
    turn.transcript
        .items()
        .into_iter()
        .filter(|item| item.item_kind() == ItemKind::AgentMessage)
        .any(|message| {
            let message = message
                .message_text()
                .filter(|text| !text.trim().is_empty())
                .unwrap_or_else(|| message.output_text())
                .trim();
            !message.is_empty()
        })
}

fn require_planning_tty() -> Result<()> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        Ok(())
    } else {
        bail!("lgtm plan requires interactive stdin and stdout; run it from a TTY")
    }
}

fn connect_client(config: &PlanConfig) -> Result<RuntimeAppServerClient> {
    config.runtime.connect_app_server(None)
}

fn set_turn_log(config: &PlanConfig, client: &mut AppServerClient, turn_number: u32) -> Result<()> {
    config
        .runtime
        .set_log_sink(client, &config.turn_log_name(turn_number), false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileSnapshot {
    Missing,
    Present { content: Vec<u8> },
}

impl FileSnapshot {
    fn capture(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(content) => Ok(Self::Present { content }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::Missing),
            Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanningArtifactsSnapshot {
    plan: FileSnapshot,
    agents: FileSnapshot,
}

impl PlanningArtifactsSnapshot {
    fn capture(config: &PlanConfig) -> Result<Self> {
        Ok(Self {
            plan: FileSnapshot::capture(&config.plan_abs())?,
            agents: FileSnapshot::capture(&config.agents_abs())?,
        })
    }

    fn is_complete_with(&self, after: &Self) -> bool {
        let plan_changed = after.plan != self.plan;
        let agents_ready = match self.agents {
            FileSnapshot::Missing => !matches!(after.agents, FileSnapshot::Missing),
            FileSnapshot::Present { .. } => true,
        };

        plan_changed && agents_ready
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn plan_args_with_root(root: PathBuf) -> PlanArgs {
        PlanArgs {
            brief: None,
            root: Some(root),
            plan_path: "PLAN.md".into(),
            codex_bin: "codex".to_string(),
            execution: crate::cli::ExecutionArgs::default(),
            log_dir: None,
            run_stamp: Some("test".to_string()),
        }
    }

    #[test]
    fn relative_log_dir_is_resolved_under_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut args = plan_args_with_root(temp.path().to_path_buf());
        args.log_dir = Some("logs".into());

        let config = PlanConfig::from_args(args).expect("config");

        assert_eq!(config.runtime.log_dir(), temp.path().join("logs"));
    }

    #[test]
    fn artifact_completion_requires_plan_change_and_missing_agents_creation() {
        let before = PlanningArtifactsSnapshot {
            plan: FileSnapshot::Missing,
            agents: FileSnapshot::Missing,
        };
        let missing_agents = PlanningArtifactsSnapshot {
            plan: FileSnapshot::Present {
                content: b"# Plan".to_vec(),
            },
            agents: FileSnapshot::Missing,
        };
        let complete = PlanningArtifactsSnapshot {
            plan: FileSnapshot::Present {
                content: b"# Plan".to_vec(),
            },
            agents: FileSnapshot::Present {
                content: b"# Agents".to_vec(),
            },
        };

        assert!(!before.is_complete_with(&missing_agents));
        assert!(before.is_complete_with(&complete));
    }

    #[test]
    fn planning_turn_captures_message_completion_and_log() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        fs::write(root.join("AGENTS.md"), "# Agents\n").expect("agents");
        let fake_codex = executable(
            temp.path(),
            r#"#!/usr/bin/env sh
set -eu
dir=$(dirname "$0")
read initialize
printf '%s\n' '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex"}}'
read initialized
read thread_start
printf '%s\n' '{"id":2,"result":{"thread":{"id":"thr-plan"}}}'
read turn_start
printf '%s\n' "$turn_start" >"$dir/turn.json"
cat >"$dir/repo/PLAN.md" <<'PLAN'
# Plan

## Phase 1 - Test

Goal: test.
PLAN
printf '%s\n' '{"id":3,"result":{"turn":{"id":"turn-plan","status":"inProgress","items":[]}}}'
printf '%s\n' '{"method":"turn/plan/updated","params":{"threadId":"thr-plan","turnId":"turn-plan","plan":[{"status":"completed","step":"Inspect repo"},{"status":"inProgress","step":"Write plan"}]}}'
printf '%s\n' '{"method":"item/started","params":{"threadId":"thr-plan","turnId":"turn-plan","item":{"type":"commandExecution","id":"cmd-1","command":"ls","status":"inProgress"}}}'
printf '%s\n' '{"method":"item/commandExecution/outputDelta","params":{"threadId":"thr-plan","turnId":"turn-plan","itemId":"cmd-1","delta":"README.md\n"}}'
printf '%s\n' '{"method":"turn/completed","params":{"threadId":"thr-plan","turn":{"id":"turn-plan","status":"completed","items":[{"type":"mcpToolCall","id":"tool-1","server":"github","tool":"get_issue","status":"completed","result":{"title":"Planning issue"}},{"type":"commandExecution","id":"cmd-1","command":"ls","status":"completed","exitCode":0,"aggregatedOutput":"README.md\n"},{"type":"agentMessage","id":"msg-1","text":"final plan written","status":"completed"}]}}}'
"#,
        );
        let mut args = plan_args_with_root(root.clone());
        args.codex_bin = fake_codex.display().to_string();
        let config = PlanConfig::from_args(args).expect("config");
        let before = PlanningArtifactsSnapshot::capture(&config).expect("snapshot");
        let mut client = connect_client(&config).expect("client");
        set_turn_log(&config, &mut client, 1).expect("log");
        let thread_id = client.start_thread().expect("thread");
        let mut rendered = Vec::new();

        let artifacts_complete = run_planning_turn(
            &config,
            &mut client,
            &thread_id,
            "planning prompt".to_string(),
            &before,
            &mut rendered,
        )
        .expect("planning turn");
        client.stop().expect("stop");

        assert!(artifacts_complete);
        let turn = fs::read_to_string(temp.path().join("turn.json")).expect("turn prompt");
        assert!(turn.contains("planning prompt"));
        let rendered = String::from_utf8(rendered).expect("rendered output");
        assert!(rendered.contains("• Planning"));
        assert!(rendered.contains("• Updated Plan"));
        assert!(rendered.contains("  ✓ Inspect repo"));
        assert!(rendered.contains("  □ Write plan"));
        assert!(rendered.contains("• Ran mcp github/get_issue"));
        assert!(rendered.contains("Planning issue"));
        assert!(rendered.contains("• Ran ls"));
        assert!(rendered.contains("  └ README.md"));
        assert!(rendered.contains("• Codex"));
        assert!(rendered.contains("  final plan written"));
        let log = fs::read_to_string(root.join(".lgtm/logs/test-plan-001.jsonl")).expect("log");
        assert!(log.contains(r#""direction":"out""#));
        assert!(log.contains(r#""direction":"in""#));
    }

    #[test]
    fn spinner_line_fits_width() {
        let rendered = crate::output::spinner::line_for_width(
            "exploring\nrepo",
            "...",
            std::time::Duration::from_secs(7),
            30,
            false,
        )
        .expect("spinner");

        assert!(rendered.contains("exploring repo"));
        assert!(rendered.contains("... 7s"));
    }

    fn executable(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("codex");
        fs::write(&path, body).expect("script");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("chmod");
        path
    }
}
