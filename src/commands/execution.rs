use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    app_server::AppServerLaunch,
    cli::{ExecutionArgs, ExecutionSandbox},
    commands::apple_container,
    rtk,
};

#[derive(Debug, Clone)]
pub(super) struct ExecutionConfig {
    sandbox: ExecutionSandbox,
    codex_bin: String,
    sandbox_image: String,
    container_bin: String,
    codex_auth_path: Option<PathBuf>,
}

impl ExecutionConfig {
    pub(super) fn from_args(codex_bin: String, args: ExecutionArgs) -> Self {
        Self {
            sandbox: args.sandbox,
            codex_bin,
            sandbox_image: args.sandbox_image,
            container_bin: args.container_bin,
            codex_auth_path: args.codex_auth_path,
        }
    }

    #[cfg(test)]
    pub(super) fn new(
        sandbox: ExecutionSandbox,
        codex_bin: impl Into<String>,
        sandbox_image: impl Into<String>,
        container_bin: impl Into<String>,
        codex_auth_path: Option<PathBuf>,
    ) -> Self {
        Self {
            sandbox,
            codex_bin: codex_bin.into(),
            sandbox_image: sandbox_image.into(),
            container_bin: container_bin.into(),
            codex_auth_path,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum ExecutionTarget {
    Host {
        codex_bin: String,
    },
    AppleContainer {
        container_bin: String,
        image: String,
        auth_path: PathBuf,
    },
}

pub(super) struct PreparedAppServer {
    pub(super) launch: AppServerLaunch,
    pub(super) cwd: String,
    pub(super) developer_instructions_suffix: Option<&'static str>,
    pub(super) resources: ExecutionResources,
}

#[derive(Debug, Default)]
pub(super) struct ExecutionResources {
    cleanup_paths: Vec<PathBuf>,
}

impl ExecutionResources {
    pub(super) fn track_cleanup_path(&mut self, path: PathBuf) -> PathBuf {
        self.cleanup_paths.push(path.clone());
        path
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.cleanup_paths.is_empty()
    }
}

impl Drop for ExecutionResources {
    fn drop(&mut self) {
        for path in &self.cleanup_paths {
            let _ = fs::remove_dir_all(path);
        }
    }
}

impl ExecutionTarget {
    pub(super) fn from_config(config: ExecutionConfig) -> Result<Self> {
        match config.sandbox {
            ExecutionSandbox::Host => Ok(Self::Host {
                codex_bin: config.codex_bin,
            }),
            ExecutionSandbox::AppleContainer => Ok(Self::AppleContainer {
                container_bin: config.container_bin,
                image: config.sandbox_image,
                auth_path: config
                    .codex_auth_path
                    .map(absolutize)
                    .transpose()?
                    .unwrap_or(apple_container::default_auth_path()?),
            }),
        }
    }

    pub(super) fn app_server_binary(&self) -> &str {
        match self {
            Self::Host { codex_bin } => codex_bin,
            Self::AppleContainer { .. } => "codex",
        }
    }

    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::Host { .. } => "host YOLO",
            Self::AppleContainer { .. } => "Apple Container",
        }
    }

    #[cfg(test)]
    pub(super) fn apple_container_details(&self) -> Option<(&str, &str, &Path)> {
        match self {
            Self::Host { .. } => None,
            Self::AppleContainer {
                container_bin,
                image,
                auth_path,
            } => Some((container_bin, image, auth_path)),
        }
    }

    pub(super) fn prepare(&self, root: &Path) -> Result<PreparedAppServer> {
        match self {
            Self::Host { codex_bin } => Ok(PreparedAppServer {
                launch: AppServerLaunch::host(codex_bin),
                cwd: root.display().to_string(),
                developer_instructions_suffix: None,
                resources: ExecutionResources::default(),
            }),
            Self::AppleContainer {
                container_bin,
                image,
                auth_path,
            } => apple_container::prepare(container_bin, image, auth_path, root),
        }
    }

    pub(super) fn rtk_developer_instructions_suffix(&self) -> Option<&'static str> {
        match self {
            Self::Host { .. } => rtk::developer_instructions_suffix(),
            Self::AppleContainer { .. } => None,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_target_prepares_host_app_server() {
        let target = ExecutionTarget::from_config(ExecutionConfig::new(
            ExecutionSandbox::Host,
            "codex-test",
            "image",
            "container",
            None,
        ))
        .expect("target");

        let prepared = target.prepare(Path::new("/repo")).expect("prepared");

        assert_eq!(target.app_server_binary(), "codex-test");
        assert_eq!(target.label(), "host YOLO");
        assert_eq!(prepared.cwd, "/repo");
        assert_eq!(prepared.launch.program(), "codex-test");
        assert_eq!(prepared.launch.args(), ["app-server"]);
        assert!(prepared.developer_instructions_suffix.is_none());
        assert!(prepared.resources.is_empty());
    }

    #[test]
    fn execution_resources_remove_tracked_paths_on_drop() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cleanup_dir = temp.path().join("cleanup");
        fs::create_dir(&cleanup_dir).expect("cleanup dir");

        let mut resources = ExecutionResources::default();
        resources.track_cleanup_path(cleanup_dir.clone());
        drop(resources);

        assert!(!cleanup_dir.exists());
    }
}
