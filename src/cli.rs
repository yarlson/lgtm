use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

pub const DEFAULT_SANDBOX_IMAGE: &str = "ghcr.io/yarlson/lgtm-codex:latest";

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Plan and run Codex-backed phase work with formatted output",
    subcommand_required = true,
    arg_required_else_help = false,
    after_long_help = "Execution policy:\n  lgtm runs Codex app-server turns with danger-full-access and approval policy never. Host execution runs inside the target root. Apple Container execution mounts the target root and a temporary Codex auth directory into the container."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run implementation, validation, review, and commit passes for plan phases.
    Run(RunArgs),
    /// Create or refine a PLAN.md through an interactive Codex planning session.
    Plan(PlanArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(long, env = "ROOT_DIR")]
    pub root: Option<PathBuf>,

    #[arg(long, env = "PLAN_PATH", default_value = "PLAN.md")]
    pub plan_path: PathBuf,

    #[arg(long, env = "REPO_AGENTS_PATH", default_value = "AGENTS.md")]
    pub agents_path: PathBuf,

    #[arg(long, env = "START_PHASE", default_value_t = 1)]
    pub start_phase: u32,

    #[arg(long, env = "END_PHASE")]
    pub end_phase: Option<u32>,

    #[arg(long, env = "SLEEP_SECONDS", default_value_t = 10)]
    pub sleep_seconds: u64,

    #[arg(long, env = "CODEX_BIN", default_value = "codex")]
    pub codex_bin: String,

    #[clap(flatten)]
    pub execution: ExecutionArgs,

    #[arg(long, env = "STREAM_MODE", default_value = "pretty")]
    pub stream_mode: StreamMode,

    #[arg(long, env = "LOG_DIR")]
    pub log_dir: Option<PathBuf>,

    #[arg(long, env = "RUN_STAMP")]
    pub run_stamp: Option<String>,
}

#[derive(Debug, Args)]
pub struct PlanArgs {
    pub brief: Option<String>,

    #[arg(long, env = "ROOT_DIR")]
    pub root: Option<PathBuf>,

    #[arg(long, env = "PLAN_PATH", default_value = "PLAN.md")]
    pub plan_path: PathBuf,

    #[arg(long, env = "CODEX_BIN", default_value = "codex")]
    pub codex_bin: String,

    #[clap(flatten)]
    pub execution: ExecutionArgs,

    #[arg(long, env = "LOG_DIR")]
    pub log_dir: Option<PathBuf>,

    #[arg(long, env = "RUN_STAMP")]
    pub run_stamp: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StreamMode {
    Pretty,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ExecutionArgs {
    #[arg(
        long = "execution-sandbox",
        env = "LGTM_EXECUTION_SANDBOX",
        default_value = "host",
        value_enum
    )]
    pub sandbox: ExecutionSandbox,

    #[arg(long, env = "LGTM_SANDBOX_IMAGE", default_value = DEFAULT_SANDBOX_IMAGE)]
    pub sandbox_image: String,

    #[arg(long, env = "CONTAINER_BIN", default_value = "container")]
    pub container_bin: String,

    #[arg(long, env = "CODEX_AUTH_PATH")]
    pub codex_auth_path: Option<PathBuf>,
}

impl Default for ExecutionArgs {
    fn default() -> Self {
        Self {
            sandbox: ExecutionSandbox::Host,
            sandbox_image: DEFAULT_SANDBOX_IMAGE.to_string(),
            container_bin: "container".to_string(),
            codex_auth_path: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExecutionSandbox {
    Host,
    AppleContainer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn parses_plan_command() {
        let cli = Cli::try_parse_from([
            "lgtm",
            "plan",
            "ship smaller phases",
            "--root",
            "/repo",
            "--plan-path",
            "PLAN.md",
            "--codex-bin",
            "codex-test",
            "--execution-sandbox",
            "apple-container",
            "--sandbox-image",
            "example.com/lgtm-codex:test",
            "--container-bin",
            "container-test",
            "--codex-auth-path",
            "/tmp/auth.json",
            "--log-dir",
            ".lgtm/logs",
            "--run-stamp",
            "test",
        ])
        .unwrap();

        let Command::Plan(args) = cli.command else {
            panic!("expected plan command");
        };
        assert_eq!(args.brief.as_deref(), Some("ship smaller phases"));
        assert_eq!(args.root.unwrap(), PathBuf::from("/repo"));
        assert_eq!(args.plan_path, PathBuf::from("PLAN.md"));
        assert_eq!(args.codex_bin, "codex-test");
        assert_eq!(args.execution.sandbox, ExecutionSandbox::AppleContainer);
        assert_eq!(args.execution.sandbox_image, "example.com/lgtm-codex:test");
        assert_eq!(args.execution.container_bin, "container-test");
        assert_eq!(
            args.execution.codex_auth_path.unwrap(),
            PathBuf::from("/tmp/auth.json")
        );
        assert_eq!(args.log_dir.unwrap(), PathBuf::from(".lgtm/logs"));
        assert_eq!(args.run_stamp.as_deref(), Some("test"));
    }

    #[test]
    fn parses_run_command() {
        let cli = Cli::try_parse_from([
            "lgtm",
            "run",
            "--root",
            "/repo",
            "--plan-path",
            "CUSTOM_PLAN.md",
            "--agents-path",
            "CUSTOM_AGENTS.md",
            "--start-phase",
            "2",
            "--end-phase",
            "3",
            "--sleep-seconds",
            "0",
            "--codex-bin",
            "codex-test",
            "--execution-sandbox",
            "apple-container",
            "--sandbox-image",
            "example.com/lgtm-codex:test",
            "--container-bin",
            "container-test",
            "--codex-auth-path",
            "/tmp/auth.json",
            "--stream-mode",
            "raw",
            "--log-dir",
            ".lgtm/logs",
            "--run-stamp",
            "test",
        ])
        .unwrap();

        let Command::Run(args) = cli.command else {
            panic!("expected run command");
        };
        assert_eq!(args.root.unwrap(), PathBuf::from("/repo"));
        assert_eq!(args.plan_path, PathBuf::from("CUSTOM_PLAN.md"));
        assert_eq!(args.agents_path, PathBuf::from("CUSTOM_AGENTS.md"));
        assert_eq!(args.start_phase, 2);
        assert_eq!(args.end_phase, Some(3));
        assert_eq!(args.sleep_seconds, 0);
        assert_eq!(args.codex_bin, "codex-test");
        assert_eq!(args.execution.sandbox, ExecutionSandbox::AppleContainer);
        assert_eq!(args.execution.sandbox_image, "example.com/lgtm-codex:test");
        assert_eq!(args.execution.container_bin, "container-test");
        assert_eq!(
            args.execution.codex_auth_path.unwrap(),
            PathBuf::from("/tmp/auth.json")
        );
        assert_eq!(args.stream_mode, StreamMode::Raw);
        assert_eq!(args.log_dir.unwrap(), PathBuf::from(".lgtm/logs"));
        assert_eq!(args.run_stamp.as_deref(), Some("test"));
    }

    #[test]
    fn run_command_defaults_sleep_seconds_to_ten() {
        let cli = Cli::try_parse_from(["lgtm", "run"]).unwrap();

        let Command::Run(args) = cli.command else {
            panic!("expected run command");
        };
        assert_eq!(args.sleep_seconds, 10);
    }

    #[test]
    fn exposes_snap_rs_compatible_help_surface() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();

        assert!(help.contains("Usage:"));
        assert!(help.contains("run"));
        assert!(help.contains("plan"));
        assert!(help.contains("Execution policy:"));

        let run_help = Cli::command()
            .find_subcommand_mut("run")
            .unwrap()
            .render_long_help()
            .to_string();
        assert!(run_help.contains("--root"));
        assert!(run_help.contains("--plan-path"));
        assert!(run_help.contains("--agents-path"));
        assert!(run_help.contains("--start-phase"));
        assert!(run_help.contains("--end-phase"));
        assert!(run_help.contains("--sleep-seconds"));
        assert!(run_help.contains("[default: 10]"));
        assert!(run_help.contains("--codex-bin"));
        assert!(run_help.contains("--execution-sandbox"));
        assert!(run_help.contains("--sandbox-image"));
        assert!(run_help.contains("--container-bin"));
        assert!(run_help.contains("--codex-auth-path"));
        assert!(run_help.contains("--stream-mode"));
        assert!(run_help.contains("--log-dir"));
        assert!(run_help.contains("--run-stamp"));

        let plan_help = Cli::command()
            .find_subcommand_mut("plan")
            .unwrap()
            .render_long_help()
            .to_string();
        assert!(plan_help.contains("[BRIEF]"));
        assert!(plan_help.contains("--root"));
        assert!(plan_help.contains("--plan-path"));
        assert!(plan_help.contains("--codex-bin"));
        assert!(plan_help.contains("--execution-sandbox"));
        assert!(plan_help.contains("--sandbox-image"));
        assert!(plan_help.contains("--container-bin"));
        assert!(plan_help.contains("--codex-auth-path"));
        assert!(plan_help.contains("--log-dir"));
        assert!(plan_help.contains("--run-stamp"));
    }
}
