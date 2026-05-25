use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};

use crate::{
    app_server::AppServerLaunch,
    cli::{ExecutionArgs, ExecutionSandbox},
};

const CONTAINER_WORKDIR: &str = "/workspace";
const CONTAINER_MISE_DIR: &str = "/mise";
const CONTAINER_PATH: &str =
    "/mise/shims:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
const SANDBOX_MISE_INSTRUCTIONS: &str = r#"Apple Container sandbox guidance:
- `mise` is installed and available on PATH. Use it to provision and activate missing project toolchains.
- First inspect the repository for stack and version signals such as `mise.toml`, `.mise.toml`, `.tool-versions`, `package.json`, `pyproject.toml`, `Cargo.toml`, `go.mod`, `Gemfile`, `pom.xml`, `build.gradle`, `deno.json`, or `bun.lock`.
- If the repo already declares mise/asdf tools, run `mise install` before using those tools.
- If a required interpreter, runtime, compiler, or package manager is missing and the repo does not declare a toolchain, activate the narrow tool needed for the detected stack with `mise use -g -y <tool>@<version>`. This writes sandbox-global config under `/mise/config.toml`, not project files.
- After `mise use -g`, run the normal command directly (`go test`, `python -m pytest`, `node`, `cargo`, etc.) because `/mise/shims` is already on PATH. Do not keep wrapping every command in `mise exec` once the tool is activated.
- Each tool call starts a fresh login shell. The sandbox image keeps `/mise/shims` on PATH through shell startup. If a direct command is unexpectedly missing after activation, check `echo "$PATH"` and `command -v <tool>`; do not solve that by adding repeated `mise exec` wrappers.
- Do not edit project toolchain config files only to make tools available unless the user's task requires that change.
- Use OS package installation only for system libraries or packages that mise cannot provide."#;

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
    fn track_cleanup_path(&mut self, path: PathBuf) -> PathBuf {
        self.cleanup_paths.push(path.clone());
        path
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
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
                    .unwrap_or(default_codex_auth_path()?),
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
            } => {
                preflight_apple_container(auth_path)?;
                let mise_dir = prepare_mise_dir(root)?;
                let mut resources = ExecutionResources::default();
                let auth_dir = resources.track_cleanup_path(prepare_codex_auth_dir(auth_path)?);
                Ok(PreparedAppServer {
                    launch: apple_container_launch(
                        container_bin,
                        root,
                        &auth_dir,
                        &mise_dir,
                        image,
                    ),
                    cwd: CONTAINER_WORKDIR.to_string(),
                    developer_instructions_suffix: Some(SANDBOX_MISE_INSTRUCTIONS),
                    resources,
                })
            }
        }
    }
}

fn apple_container_launch(
    container_bin: &str,
    root: &Path,
    auth_dir: &Path,
    mise_dir: &Path,
    image: &str,
) -> AppServerLaunch {
    AppServerLaunch::new(
        container_bin,
        [
            "run".to_string(),
            "--rm".to_string(),
            "-i".to_string(),
            "--progress".to_string(),
            "none".to_string(),
            "--workdir".to_string(),
            CONTAINER_WORKDIR.to_string(),
            "--mount".to_string(),
            format!("type=bind,source={},target=/workspace", root.display()),
            "--mount".to_string(),
            format!(
                "type=bind,source={},target=/root/.codex",
                auth_dir.display()
            ),
            "--mount".to_string(),
            format!(
                "type=bind,source={},target={CONTAINER_MISE_DIR}",
                mise_dir.display()
            ),
            "--env".to_string(),
            "HOME=/root".to_string(),
            "--env".to_string(),
            "CODEX_HOME=/root/.codex".to_string(),
            "--env".to_string(),
            format!("MISE_DATA_DIR={CONTAINER_MISE_DIR}"),
            "--env".to_string(),
            format!("MISE_CONFIG_DIR={CONTAINER_MISE_DIR}"),
            "--env".to_string(),
            format!("MISE_CACHE_DIR={CONTAINER_MISE_DIR}/cache"),
            "--env".to_string(),
            "MISE_PIN=1".to_string(),
            "--env".to_string(),
            format!("PATH={CONTAINER_PATH}"),
            image.to_string(),
            "codex".to_string(),
            "app-server".to_string(),
        ],
    )
}

fn absolutize(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(std::env::current_dir()
        .context("failed to read current directory")?
        .join(path))
}

fn default_codex_auth_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set; pass --codex-auth-path")?;
    Ok(PathBuf::from(home).join(".codex").join("auth.json"))
}

fn preflight_apple_container(auth_path: &Path) -> Result<()> {
    if std::env::consts::OS != "macos" {
        bail!("Apple Container sandbox requires macOS");
    }
    if std::env::consts::ARCH != "aarch64" {
        bail!("Apple Container sandbox requires Apple silicon");
    }
    if !auth_path.is_file() {
        bail!(
            "Codex auth file {} was not found; pass --codex-auth-path",
            auth_path.display()
        );
    }
    Ok(())
}

fn prepare_codex_auth_dir(auth_path: &Path) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "lgtm-codex-auth-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before Unix epoch")?
            .as_nanos()
    ));
    fs::create_dir(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let copied_auth = dir.join("auth.json");
    if let Err(error) = fs::copy(auth_path, &copied_auth)
        .with_context(|| format!("failed to copy Codex auth file {}", auth_path.display()))
    {
        let _ = fs::remove_dir_all(&dir);
        return Err(error);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(&copied_auth, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", copied_auth.display()))?;
    }
    Ok(dir)
}

fn prepare_mise_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(".codex-log").join("mise");
    fs::create_dir_all(dir.join("cache"))
        .with_context(|| format!("failed to create {}", dir.join("cache").display()))?;
    fs::create_dir_all(dir.join("shims"))
        .with_context(|| format!("failed to create {}", dir.join("shims").display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_container_launch_wraps_codex_app_server() {
        let launch = apple_container_launch(
            "container-test",
            Path::new("/repo"),
            Path::new("/tmp/lgtm-codex-auth"),
            Path::new("/repo/.codex-log/mise"),
            "example.com/lgtm-codex:test",
        );

        assert_eq!(launch.program(), "container-test");
        assert_eq!(
            launch.args(),
            [
                "run",
                "--rm",
                "-i",
                "--progress",
                "none",
                "--workdir",
                "/workspace",
                "--mount",
                "type=bind,source=/repo,target=/workspace",
                "--mount",
                "type=bind,source=/tmp/lgtm-codex-auth,target=/root/.codex",
                "--mount",
                "type=bind,source=/repo/.codex-log/mise,target=/mise",
                "--env",
                "HOME=/root",
                "--env",
                "CODEX_HOME=/root/.codex",
                "--env",
                "MISE_DATA_DIR=/mise",
                "--env",
                "MISE_CONFIG_DIR=/mise",
                "--env",
                "MISE_CACHE_DIR=/mise/cache",
                "--env",
                "MISE_PIN=1",
                "--env",
                "PATH=/mise/shims:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                "example.com/lgtm-codex:test",
                "codex",
                "app-server",
            ]
        );
    }

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
    fn apple_container_prepare_without_preflight_uses_repo_mise_dir() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let auth_dir = temp.path().join("auth");
        let mise_dir = root.join(".codex-log").join("mise");

        let launch = apple_container_launch(
            "container-test",
            &root,
            &auth_dir,
            &mise_dir,
            "example.com/lgtm-codex:test",
        );

        assert_eq!(launch.program(), "container-test");
        assert!(launch.args().contains(&format!(
            "type=bind,source={},target=/mise",
            mise_dir.display()
        )));
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

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn apple_container_target_prepares_mise_mount_and_instructions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");
        let target = ExecutionTarget::from_config(ExecutionConfig::new(
            ExecutionSandbox::AppleContainer,
            "codex",
            "example.com/lgtm-codex:test",
            "container-test",
            Some(auth_path),
        ))
        .expect("target");

        let prepared = target.prepare(&root).expect("prepared");

        assert_eq!(target.app_server_binary(), "codex");
        assert_eq!(target.label(), "Apple Container");
        assert_eq!(prepared.cwd, CONTAINER_WORKDIR);
        assert!(
            prepared
                .developer_instructions_suffix
                .is_some_and(|text| text.contains("mise use -g"))
        );
        assert!(root.join(".codex-log/mise/cache").is_dir());
        assert!(root.join(".codex-log/mise/shims").is_dir());
        assert!(prepared.launch.args().contains(&format!(
            "type=bind,source={},target=/mise",
            root.join(".codex-log/mise").display()
        )));
        assert!(!prepared.resources.is_empty());
    }

    #[test]
    fn default_auth_path_uses_home() {
        let home = std::env::var_os("HOME").expect("home");

        assert_eq!(
            default_codex_auth_path().expect("auth path"),
            PathBuf::from(home).join(".codex").join("auth.json")
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn apple_container_preflight_reports_missing_auth_file() {
        let err =
            preflight_apple_container(Path::new("/tmp/lgtm-definitely-missing-codex-auth.json"))
                .unwrap_err();

        assert!(err.to_string().contains("Codex auth file"));
    }
}
