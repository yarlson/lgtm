use std::path::PathBuf;

use chrono::Local;
use clap::Args as ClapArgs;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;

use crate::Error;

#[derive(Debug, Clone, Parser)]
#[command(
    version,
    about = "Run a Codex-backed phase plan with formatted JSONL output",
    subcommand_required = true,
    arg_required_else_help = false,
    after_long_help = "Execution policy:\n  lgtm runs `codex exec` with `--dangerously-bypass-approvals-and-sandbox` inside the target root. Use it only for repositories where that level of local filesystem and command execution autonomy is acceptable."
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    /// Run implementation, validation, and review passes for plan phases.
    Run(RunArgs),
    /// Create or refine a PLAN.md through an interactive Codex planning session.
    Plan(PlanArgs),
}

#[derive(Debug, Clone, ClapArgs)]
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

    #[arg(long, env = "SLEEP_SECONDS", default_value_t = 600)]
    pub sleep_seconds: u64,

    #[arg(long, env = "CODEX_BIN", default_value = "codex")]
    pub codex_bin: String,

    #[arg(long, env = "STREAM_MODE", default_value = "pretty")]
    pub stream_mode: StreamMode,

    #[arg(long, env = "LOG_DIR")]
    pub log_dir: Option<PathBuf>,

    #[arg(long, env = "RUN_STAMP")]
    pub run_stamp: Option<String>,
}

#[derive(Debug, Clone, ClapArgs)]
pub struct PlanArgs {
    pub brief: Option<String>,

    #[arg(long, env = "ROOT_DIR")]
    pub root: Option<PathBuf>,

    #[arg(long, env = "PLAN_PATH", default_value = "PLAN.md")]
    pub plan_path: PathBuf,

    #[arg(long, env = "CODEX_BIN", default_value = "codex")]
    pub codex_bin: String,

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

#[derive(Debug, Clone)]
pub struct Config {
    pub root: PathBuf,
    pub plan_path: PathBuf,
    pub agents_path: PathBuf,
    pub start_phase: u32,
    pub end_phase: Option<u32>,
    pub sleep_seconds: u64,
    pub codex_bin: String,
    pub stream_mode: StreamMode,
    pub log_dir: PathBuf,
    pub run_stamp: String,
}

impl Config {
    pub fn from_args(args: RunArgs) -> Result<Self, Error> {
        let common = CommonConfig::from_parts(args.root, args.log_dir, args.run_stamp)?;

        Ok(Self {
            root: common.root,
            plan_path: args.plan_path,
            agents_path: args.agents_path,
            start_phase: args.start_phase,
            end_phase: args.end_phase,
            sleep_seconds: args.sleep_seconds,
            codex_bin: args.codex_bin,
            stream_mode: args.stream_mode,
            log_dir: common.log_dir,
            run_stamp: common.run_stamp,
        })
    }

    pub fn plan_abs(&self) -> PathBuf {
        self.root.join(&self.plan_path)
    }

    pub fn agents_abs(&self) -> PathBuf {
        self.root.join(&self.agents_path)
    }
}

#[derive(Debug, Clone)]
pub struct PlanConfig {
    pub root: PathBuf,
    pub plan_path: PathBuf,
    pub brief: Option<String>,
    pub codex_bin: String,
    pub log_dir: PathBuf,
    pub run_stamp: String,
}

impl PlanConfig {
    pub fn from_args(args: PlanArgs) -> Result<Self, Error> {
        let common = CommonConfig::from_parts(args.root, args.log_dir, args.run_stamp)?;

        Ok(Self {
            root: common.root,
            plan_path: args.plan_path,
            brief: args.brief,
            codex_bin: args.codex_bin,
            log_dir: common.log_dir,
            run_stamp: common.run_stamp,
        })
    }

    pub fn plan_abs(&self) -> PathBuf {
        self.root.join(&self.plan_path)
    }
}

struct CommonConfig {
    root: PathBuf,
    log_dir: PathBuf,
    run_stamp: String,
}

impl CommonConfig {
    fn from_parts(
        root: Option<PathBuf>,
        log_dir: Option<PathBuf>,
        run_stamp: Option<String>,
    ) -> Result<Self, Error> {
        let root = match root {
            Some(root) => absolutize(root)?,
            None => std::env::current_dir().map_err(|source| Error::io(".", source))?,
        };
        let log_dir = log_dir
            .map(|path| resolve_under_root(&root, path))
            .unwrap_or_else(|| root.join(".codex-log"));
        let run_stamp =
            run_stamp.unwrap_or_else(|| Local::now().format("%Y%m%d-%H%M%S").to_string());

        Ok(Self {
            root,
            log_dir,
            run_stamp,
        })
    }
}

fn absolutize(path: PathBuf) -> Result<PathBuf, Error> {
    if path.is_absolute() {
        return Ok(path);
    }
    let cwd = std::env::current_dir().map_err(|source| Error::io(".", source))?;
    Ok(cwd.join(path))
}

fn resolve_under_root(root: &std::path::Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::Parser;

    fn run_args_with_root(root: PathBuf) -> RunArgs {
        RunArgs {
            root: Some(root),
            plan_path: "PLAN.md".into(),
            agents_path: "AGENTS.md".into(),
            start_phase: 1,
            end_phase: Some(1),
            sleep_seconds: 0,
            codex_bin: "codex".to_string(),
            stream_mode: StreamMode::Pretty,
            log_dir: None,
            run_stamp: Some("test".to_string()),
        }
    }

    fn plan_args_with_root(root: PathBuf) -> PlanArgs {
        PlanArgs {
            brief: None,
            root: Some(root),
            plan_path: "PLAN.md".into(),
            codex_bin: "codex".to_string(),
            log_dir: None,
            run_stamp: Some("test".to_string()),
        }
    }

    #[test]
    fn run_relative_log_dir_is_resolved_under_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut args = run_args_with_root(temp.path().to_path_buf());
        args.log_dir = Some("logs".into());

        let config = Config::from_args(args).expect("config");

        assert_eq!(config.log_dir, temp.path().join("logs"));
    }

    #[test]
    fn run_absolute_log_dir_is_preserved() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_dir = temp.path().join("outside");
        let mut args = run_args_with_root(temp.path().join("repo"));
        args.log_dir = Some(log_dir.clone());

        let config = Config::from_args(args).expect("config");

        assert_eq!(config.log_dir, log_dir);
    }

    #[test]
    fn run_subcommand_owns_phase_options() {
        let args = Args::try_parse_from([
            "lgtm",
            "run",
            "--start-phase",
            "2",
            "--end-phase",
            "3",
            "--sleep-seconds",
            "0",
        ])
        .expect("parse run command");

        let Command::Run(run) = args.command else {
            panic!("expected run command");
        };
        assert_eq!(run.start_phase, 2);
        assert_eq!(run.end_phase, Some(3));
        assert_eq!(run.sleep_seconds, 0);
    }

    #[test]
    fn bare_legacy_phase_options_are_rejected() {
        let error = Args::try_parse_from(["lgtm", "--start-phase", "1"])
            .expect_err("legacy bare phase options should fail");

        assert!(error.to_string().contains("Usage: lgtm <COMMAND>"));
    }

    #[test]
    fn missing_subcommand_is_rejected_directly() {
        let error = Args::try_parse_from(["lgtm"]).expect_err("subcommand should be required");

        assert!(
            error
                .to_string()
                .contains("requires a subcommand but one was not provided")
        );
    }

    #[test]
    fn run_help_shows_phase_run_options() {
        let help = subcommand_help("run");

        for option in [
            "--root",
            "--plan-path",
            "--agents-path",
            "--start-phase",
            "--end-phase",
            "--sleep-seconds",
            "--codex-bin",
            "--stream-mode",
            "--log-dir",
            "--run-stamp",
        ] {
            assert!(help.contains(option), "run help should contain {option}");
        }
    }

    #[test]
    fn plan_help_shows_planning_options_and_optional_brief() {
        let help = subcommand_help("plan");

        for option in [
            "[BRIEF]",
            "--root",
            "--plan-path",
            "--codex-bin",
            "--log-dir",
            "--run-stamp",
        ] {
            assert!(help.contains(option), "plan help should contain {option}");
        }

        for run_only_option in ["--agents-path", "--start-phase", "--end-phase"] {
            assert!(
                !help.contains(run_only_option),
                "plan help should not contain run-only option {run_only_option}"
            );
        }
    }

    #[test]
    fn plan_subcommand_accepts_optional_brief_and_shared_options() {
        let args = Args::try_parse_from([
            "lgtm",
            "plan",
            "ship smaller phases",
            "--root",
            "/tmp/repo",
            "--plan-path",
            "docs/PLAN.md",
            "--codex-bin",
            "codex-dev",
            "--log-dir",
            "logs",
            "--run-stamp",
            "test",
        ])
        .expect("parse plan command");

        let Command::Plan(plan) = args.command else {
            panic!("expected plan command");
        };
        assert_eq!(plan.brief.as_deref(), Some("ship smaller phases"));
        assert_eq!(
            plan.root.as_deref(),
            Some(std::path::Path::new("/tmp/repo"))
        );
        assert_eq!(plan.plan_path, PathBuf::from("docs/PLAN.md"));
        assert_eq!(plan.codex_bin, "codex-dev");
        assert_eq!(plan.log_dir.as_deref(), Some(std::path::Path::new("logs")));
        assert_eq!(plan.run_stamp.as_deref(), Some("test"));
    }

    #[test]
    fn plan_config_does_not_require_run_only_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut args = plan_args_with_root(temp.path().to_path_buf());
        args.log_dir = Some("logs".into());
        args.brief = Some("tight plan".to_string());

        let config = PlanConfig::from_args(args).expect("config");

        assert_eq!(config.root, temp.path());
        assert_eq!(config.plan_path, PathBuf::from("PLAN.md"));
        assert_eq!(config.plan_abs(), temp.path().join("PLAN.md"));
        assert_eq!(config.log_dir, temp.path().join("logs"));
        assert_eq!(config.brief.as_deref(), Some("tight plan"));
        assert_eq!(config.codex_bin, "codex");
        assert_eq!(config.run_stamp, "test");
    }

    fn subcommand_help(name: &str) -> String {
        let mut command = Args::command();
        command
            .find_subcommand_mut(name)
            .expect("subcommand")
            .render_long_help()
            .to_string()
    }
}
