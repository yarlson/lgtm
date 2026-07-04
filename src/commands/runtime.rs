use std::{
    fs::{self, File},
    io::Write,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, bail};
use chrono::Local;

use crate::{
    app_server::{AppServerClient, AppServerConfig},
    commands::execution::{
        ExecutionConfig, ExecutionResources, ExecutionTarget, PreparedAppServer,
    },
    paths,
};

#[derive(Debug, Clone)]
pub(super) struct CommandRuntime {
    root: PathBuf,
    log_dir: PathBuf,
    run_stamp: String,
    execution: ExecutionTarget,
}

pub(super) struct CommandRuntimeConfig {
    pub(super) root: Option<PathBuf>,
    pub(super) log_dir: Option<PathBuf>,
    pub(super) run_stamp: Option<String>,
    pub(super) execution: ExecutionConfig,
}

impl CommandRuntime {
    pub(super) fn new(config: CommandRuntimeConfig) -> Result<Self> {
        let CommandRuntimeConfig {
            root,
            log_dir,
            run_stamp,
            execution,
        } = config;
        let root = match root {
            Some(root) => absolutize(root)?,
            None => std::env::current_dir().context("failed to read current directory")?,
        };
        let log_dir = log_dir
            .map(|path| resolve_under_root(&root, path))
            .unwrap_or_else(|| paths::default_log_dir(&root));
        let run_stamp =
            run_stamp.unwrap_or_else(|| Local::now().format("%Y%m%d-%H%M%S").to_string());
        let execution = ExecutionTarget::from_config(execution)?;

        Ok(Self {
            root,
            log_dir,
            run_stamp,
            execution,
        })
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn app_server_binary(&self) -> &str {
        self.execution.app_server_binary()
    }

    pub(super) fn execution_label(&self) -> &'static str {
        self.execution.label()
    }

    pub(super) fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    pub(super) fn run_stamp(&self) -> &str {
        &self.run_stamp
    }

    #[cfg(test)]
    pub(super) fn apple_container_execution_details(&self) -> Option<(&str, &str, &Path)> {
        self.execution.apple_container_details()
    }

    pub(super) fn resolve_root_path(&self, path: &Path) -> PathBuf {
        self.root.join(path)
    }

    pub(super) fn connect_app_server(
        &self,
        model: Option<String>,
    ) -> Result<RuntimeAppServerClient> {
        let prepared = self.prepare_app_server(model)?;
        let client = AppServerClient::connect(prepared.launch, prepared.config)?;
        Ok(RuntimeAppServerClient {
            client,
            _resources: prepared.resources,
        })
    }

    pub(super) fn connect_logged_app_server(
        &self,
        model: Option<String>,
        log_name: &str,
        echo_raw_stdout: bool,
    ) -> Result<RuntimeAppServerClient> {
        let mut client = self.connect_app_server(model)?;
        self.set_log_sink(&mut client.client, log_name, echo_raw_stdout)?;
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

    fn prepare_app_server(&self, model: Option<String>) -> Result<RuntimeAppServer> {
        let PreparedAppServer {
            launch,
            cwd,
            developer_instructions_suffix,
            resources,
        } = self.execution.prepare(&self.root)?;
        let mut config = AppServerConfig::for_run(cwd, model);
        let rtk_developer_instructions_suffix = self.execution.rtk_developer_instructions_suffix();
        let suffixes = [
            developer_instructions_suffix,
            rtk_developer_instructions_suffix,
        ];
        if suffixes.iter().any(Option::is_some) {
            let mut developer_instructions = config.developer_instructions.clone();
            for suffix in suffixes.into_iter().flatten() {
                developer_instructions.push_str("\n\n");
                developer_instructions.push_str(suffix);
            }
            config = config.with_developer_instructions(developer_instructions);
        }
        Ok(RuntimeAppServer {
            launch,
            config,
            resources,
        })
    }
}

pub(super) struct RuntimeAppServerClient {
    client: AppServerClient,
    _resources: ExecutionResources,
}

impl RuntimeAppServerClient {
    pub(super) fn stop(self) -> Result<()> {
        let Self { client, _resources } = self;
        client.stop()
    }
}

impl Deref for RuntimeAppServerClient {
    type Target = AppServerClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for RuntimeAppServerClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

struct RuntimeAppServer {
    launch: crate::app_server::AppServerLaunch,
    config: AppServerConfig,
    resources: ExecutionResources,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_with_root(
        root: PathBuf,
        execution_sandbox: crate::cli::ExecutionSandbox,
    ) -> CommandRuntime {
        CommandRuntime::new(CommandRuntimeConfig {
            root: Some(root),
            log_dir: None,
            run_stamp: Some("test".to_string()),
            execution: ExecutionConfig::new(
                execution_sandbox,
                "codex",
                "example.com/lgtm-codex:test",
                "container-test",
                Some("/tmp/codex-auth.json".into()),
            ),
        })
        .expect("runtime")
    }

    #[test]
    fn host_app_server_config_uses_host_root() {
        let runtime = runtime_with_root(PathBuf::from("/repo"), crate::cli::ExecutionSandbox::Host);

        let prepared = runtime.prepare_app_server(None).expect("config");

        assert_eq!(prepared.config.cwd, "/repo");
        assert_eq!(prepared.launch.program(), "codex");
        assert_eq!(prepared.launch.args(), ["app-server"]);
        assert_eq!(runtime.app_server_binary(), "codex");
    }

    #[test]
    fn default_log_dir_uses_lgtm_logs() {
        let runtime = runtime_with_root(PathBuf::from("/repo"), crate::cli::ExecutionSandbox::Host);

        assert_eq!(runtime.log_dir(), Path::new("/repo/.lgtm/logs"));
    }

    #[test]
    fn apple_container_runtime_records_container_target() {
        let runtime = runtime_with_root(
            PathBuf::from("/repo"),
            crate::cli::ExecutionSandbox::AppleContainer,
        );

        assert_eq!(runtime.execution_label(), "Apple Container");
        assert_eq!(runtime.app_server_binary(), "codex");
    }
}
