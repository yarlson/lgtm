use std::path::PathBuf;

use chrono::Local;
use clap::Parser;
use clap::ValueEnum;

use crate::Error;

#[derive(Debug, Clone, Parser)]
#[command(
    version,
    about = "Run a Codex-backed phase plan with formatted JSONL output",
    after_long_help = "Execution policy:\n  snap-rs runs `codex exec` with `--dangerously-bypass-approvals-and-sandbox` inside the target root. Use it only for repositories where that level of local filesystem and command execution autonomy is acceptable."
)]
pub struct Args {
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
    pub fn from_args(args: Args) -> Result<Self, Error> {
        let root = match args.root {
            Some(root) => absolutize(root)?,
            None => std::env::current_dir().map_err(|source| Error::io(".", source))?,
        };
        let log_dir = args
            .log_dir
            .map(|path| resolve_under_root(&root, path))
            .unwrap_or_else(|| root.join(".codex-log"));
        let run_stamp = args
            .run_stamp
            .unwrap_or_else(|| Local::now().format("%Y%m%d-%H%M%S").to_string());

        Ok(Self {
            root,
            plan_path: args.plan_path,
            agents_path: args.agents_path,
            start_phase: args.start_phase,
            end_phase: args.end_phase,
            sleep_seconds: args.sleep_seconds,
            codex_bin: args.codex_bin,
            stream_mode: args.stream_mode,
            log_dir,
            run_stamp,
        })
    }

    pub fn plan_abs(&self) -> PathBuf {
        self.root.join(&self.plan_path)
    }

    pub fn agents_abs(&self) -> PathBuf {
        self.root.join(&self.agents_path)
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

    fn args_with_root(root: PathBuf) -> Args {
        Args {
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

    #[test]
    fn relative_log_dir_is_resolved_under_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut args = args_with_root(temp.path().to_path_buf());
        args.log_dir = Some("logs".into());

        let config = Config::from_args(args).expect("config");

        assert_eq!(config.log_dir, temp.path().join("logs"));
    }

    #[test]
    fn absolute_log_dir_is_preserved() {
        let temp = tempfile::tempdir().expect("tempdir");
        let log_dir = temp.path().join("outside");
        let mut args = args_with_root(temp.path().join("repo"));
        args.log_dir = Some(log_dir.clone());

        let config = Config::from_args(args).expect("config");

        assert_eq!(config.log_dir, log_dir);
    }
}
