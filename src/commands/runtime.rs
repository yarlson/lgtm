use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, bail};
use chrono::Local;

use crate::app_server::{AppServerClient, AppServerConfig};

#[derive(Debug, Clone)]
pub(super) struct CommandRuntime {
    root: PathBuf,
    codex_bin: String,
    log_dir: PathBuf,
    run_stamp: String,
}

impl CommandRuntime {
    pub(super) fn new(
        root: Option<PathBuf>,
        codex_bin: String,
        log_dir: Option<PathBuf>,
        run_stamp: Option<String>,
    ) -> Result<Self> {
        let root = match root {
            Some(root) => absolutize(root)?,
            None => std::env::current_dir().context("failed to read current directory")?,
        };
        let log_dir = log_dir
            .map(|path| resolve_under_root(&root, path))
            .unwrap_or_else(|| root.join(".codex-log"));
        let run_stamp =
            run_stamp.unwrap_or_else(|| Local::now().format("%Y%m%d-%H%M%S").to_string());

        Ok(Self {
            root,
            codex_bin,
            log_dir,
            run_stamp,
        })
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    #[cfg(test)]
    pub(super) fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    pub(super) fn run_stamp(&self) -> &str {
        &self.run_stamp
    }

    pub(super) fn resolve_root_path(&self, path: &Path) -> PathBuf {
        self.root.join(path)
    }

    pub(super) fn connect_app_server(&self, model: Option<String>) -> Result<AppServerClient> {
        AppServerClient::connect(AppServerConfig::for_run(&self.codex_bin, &self.root, model))
    }

    pub(super) fn connect_logged_app_server(
        &self,
        model: Option<String>,
        log_name: &str,
        echo_raw_stdout: bool,
    ) -> Result<AppServerClient> {
        let mut client = self.connect_app_server(model)?;
        self.set_log_sink(&mut client, log_name, echo_raw_stdout)?;
        Ok(client)
    }

    pub(super) fn set_log_sink(
        &self,
        client: &mut AppServerClient,
        log_name: &str,
        echo_raw_stdout: bool,
    ) -> Result<()> {
        fs::create_dir_all(&self.log_dir)
            .with_context(|| format!("failed to create {}", self.log_dir.display()))?;
        let log_path = self.log_dir.join(log_name);
        let log =
            Arc::new(Mutex::new(File::create(&log_path).with_context(|| {
                format!("failed to create {}", log_path.display())
            })?));
        let log_for_sink = Arc::clone(&log);
        client.log_raw_messages(move |line| {
            log_for_sink
                .lock()
                .expect("log mutex should not be poisoned")
                .write_all(line.as_bytes())
                .context("failed to write app-server log")?;
            if echo_raw_stdout {
                std::io::stdout()
                    .write_all(line.as_bytes())
                    .context("failed to write raw app-server output")?;
            }
            Ok(())
        });
        Ok(())
    }
}

pub(super) fn require_file(path: &Path, display: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        bail!("required file {} was not found", display.display())
    }
}

fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(std::env::current_dir()
        .context("failed to read current directory")?
        .join(path))
}

fn resolve_under_root(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}
