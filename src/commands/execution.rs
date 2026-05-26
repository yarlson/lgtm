use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{self, Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};

use crate::{
    app_server::AppServerLaunch,
    cli::{ExecutionArgs, ExecutionSandbox},
    paths,
};

const CONTAINER_WORKDIR: &str = "/workspace";
const CONTAINER_MISE_DIR: &str = "/mise";
const CONTAINER_PATH: &str =
    "/mise/shims:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
const SUPPORTED_MACOS_MAJOR: u32 = 26;
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
        self.prepare_with_apple_container_preflight(root, preflight_apple_container)
    }

    fn prepare_with_apple_container_preflight<F>(
        &self,
        root: &Path,
        preflight: F,
    ) -> Result<PreparedAppServer>
    where
        F: FnOnce(&str, &str, &Path) -> Result<()>,
    {
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
                preflight(container_bin, image, auth_path)?;
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

fn preflight_apple_container(container_bin: &str, image: &str, auth_path: &Path) -> Result<()> {
    let platform = HostPlatform::detect()?;
    preflight_apple_container_with_runner(container_bin, image, auth_path, platform, run_command)
}

fn preflight_apple_container_with_runner<F>(
    container_bin: &str,
    image: &str,
    auth_path: &Path,
    platform: HostPlatform,
    mut run: F,
) -> Result<()>
where
    F: FnMut(&str, &[&str]) -> io::Result<Output>,
{
    if platform.os != "macos" {
        bail!(
            "Apple Container sandbox requires macOS {SUPPORTED_MACOS_MAJOR} or newer on Apple silicon; this host is {}/{}",
            platform.os,
            platform.arch
        );
    }
    if platform.arch != "aarch64" {
        bail!(
            "Apple Container sandbox requires Apple silicon; this host architecture is {}",
            platform.arch
        );
    }
    let Some(macos_major) = platform.macos_major else {
        bail!(
            "Apple Container sandbox requires macOS {SUPPORTED_MACOS_MAJOR} or newer; failed to determine the macOS version"
        );
    };
    if macos_major < SUPPORTED_MACOS_MAJOR {
        bail!(
            "Apple Container sandbox requires macOS {SUPPORTED_MACOS_MAJOR} or newer; this host is macOS {macos_major}"
        );
    }
    if !auth_path.is_file() {
        bail!(
            "Codex auth file {} was not found; pass --codex-auth-path",
            auth_path.display()
        );
    }

    let status_args = ["system", "status"];
    let status_output = run(container_bin, &status_args)
        .map_err(|error| missing_container_cli_error(container_bin, &status_args, error))?;
    if !status_output.status.success() {
        bail!(
            "Apple Container preflight failed: container services are not running.\nRun:\n  {}\nThen retry lgtm.\n{}",
            format_shell_command(container_bin, &["system", "start"]),
            command_output_summary(container_bin, &status_args, &status_output)
        );
    }

    let inspect_args = ["image", "inspect", image];
    let inspect_output = run(container_bin, &inspect_args)
        .map_err(|error| missing_container_cli_error(container_bin, &inspect_args, error))?;
    if inspect_output.status.success() {
        return Ok(());
    }

    let pull_args = ["image", "pull", "--progress", "none", image];
    let pull_output = run(container_bin, &pull_args)
        .map_err(|error| missing_container_cli_error(container_bin, &pull_args, error))?;
    if !pull_output.status.success() {
        bail!(
            "Apple Container preflight failed: sandbox image `{image}` is not available locally and could not be pulled.\nRun:\n  {}\nThen retry lgtm.\n{}\n{}",
            format_shell_command(container_bin, &pull_args),
            command_output_summary(container_bin, &inspect_args, &inspect_output),
            command_output_summary(container_bin, &pull_args, &pull_output)
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct HostPlatform {
    os: &'static str,
    arch: &'static str,
    macos_major: Option<u32>,
}

impl HostPlatform {
    fn detect() -> Result<Self> {
        let mut platform = Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            macos_major: None,
        };
        if platform.os == "macos" {
            platform.macos_major = Some(read_macos_major()?);
        }
        Ok(platform)
    }
}

fn read_macos_major() -> Result<u32> {
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .context("failed to run `sw_vers -productVersion`")?;
    if !output.status.success() {
        bail!(
            "`sw_vers -productVersion` failed while checking Apple Container prerequisites: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let version = String::from_utf8_lossy(&output.stdout);
    parse_macos_major(&version)
        .with_context(|| format!("failed to parse macOS version from `{}`", version.trim()))
}

fn parse_macos_major(version: &str) -> Option<u32> {
    version.trim().split('.').next()?.parse().ok()
}

fn run_command(program: &str, args: &[&str]) -> io::Result<Output> {
    Command::new(program).args(args).output()
}

fn missing_container_cli_error(
    container_bin: &str,
    attempted_args: &[&str],
    error: io::Error,
) -> anyhow::Error {
    anyhow::anyhow!(
        "Apple Container preflight failed: configured container CLI `{}` could not be executed while running `{}`: {error}\nInstall Apple Container, then run:\n  {}\nIf it is already installed outside PATH, rerun lgtm with:\n  lgtm run --execution-sandbox apple-container --container-bin /path/to/container",
        container_bin,
        format_shell_command(container_bin, attempted_args),
        format_shell_command(container_bin, &["system", "start"])
    )
}

fn command_output_summary(program: &str, args: &[&str], output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if !stderr.trim().is_empty() {
        truncate_command_output(stderr.trim())
    } else if !stdout.trim().is_empty() {
        truncate_command_output(stdout.trim())
    } else {
        "no command output".to_string()
    };
    format!(
        "`{}` exited with {}: {}",
        format_shell_command(program, args),
        output.status,
        detail
    )
}

fn truncate_command_output(output: &str) -> String {
    const MAX_LEN: usize = 1000;
    if output.len() <= MAX_LEN {
        return output.to_string();
    }
    let end = output
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= MAX_LEN)
        .last()
        .unwrap_or(0);
    format!("{}...", &output[..end])
}

fn format_shell_command(program: &str, args: &[&str]) -> String {
    std::iter::once(program)
        .chain(args.iter().copied())
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b':' | b'=')
        })
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
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
    let dir = paths::sandbox_mise_dir(root);
    fs::create_dir_all(dir.join("cache"))
        .with_context(|| format!("failed to create {}", dir.join("cache").display()))?;
    fs::create_dir_all(dir.join("shims"))
        .with_context(|| format!("failed to create {}", dir.join("shims").display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    fn macos_26_arm64() -> HostPlatform {
        HostPlatform {
            os: "macos",
            arch: "aarch64",
            macos_major: Some(SUPPORTED_MACOS_MAJOR),
        }
    }

    fn command_output(code: i32, stderr: &str) -> Output {
        Output {
            status: process::ExitStatus::from_raw(code << 8),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    fn success_output() -> Output {
        command_output(0, "")
    }

    #[test]
    fn apple_container_launch_wraps_codex_app_server() {
        let launch = apple_container_launch(
            "container-test",
            Path::new("/repo"),
            Path::new("/tmp/lgtm-codex-auth"),
            Path::new("/repo/.lgtm/sandbox/mise"),
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
                "type=bind,source=/repo/.lgtm/sandbox/mise,target=/mise",
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
    fn apple_container_prepare_without_preflight_uses_sandbox_mise_dir() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let auth_dir = temp.path().join("auth");
        let mise_dir = root.join(".lgtm").join("sandbox").join("mise");

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
            Some(auth_path.clone()),
        ))
        .expect("target");

        let prepared = target
            .prepare_with_apple_container_preflight(&root, |container_bin, image, auth| {
                assert_eq!(container_bin, "container-test");
                assert_eq!(image, "example.com/lgtm-codex:test");
                assert_eq!(auth, auth_path.as_path());
                Ok(())
            })
            .expect("prepared");

        assert_eq!(target.app_server_binary(), "codex");
        assert_eq!(target.label(), "Apple Container");
        assert_eq!(prepared.cwd, CONTAINER_WORKDIR);
        assert!(
            prepared
                .developer_instructions_suffix
                .is_some_and(|text| text.contains("mise use -g"))
        );
        assert!(root.join(".lgtm/sandbox/mise/cache").is_dir());
        assert!(root.join(".lgtm/sandbox/mise/shims").is_dir());
        assert!(prepared.launch.args().contains(&format!(
            "type=bind,source={},target=/mise",
            root.join(".lgtm/sandbox/mise").display()
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

    #[test]
    fn apple_container_preflight_reports_missing_auth_file() {
        let err = preflight_apple_container_with_runner(
            "container",
            "example.com/lgtm-codex:test",
            Path::new("/tmp/lgtm-definitely-missing-codex-auth.json"),
            macos_26_arm64(),
            |_, _| Ok(success_output()),
        )
        .unwrap_err();

        assert!(err.to_string().contains("Codex auth file"));
    }

    #[test]
    fn parses_macos_major_version() {
        assert_eq!(parse_macos_major("26.0.1\n"), Some(26));
        assert_eq!(parse_macos_major("15"), Some(15));
        assert_eq!(parse_macos_major("not-a-version"), None);
    }

    #[test]
    fn apple_container_preflight_rejects_unsupported_platform_before_commands() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");
        let mut calls = Vec::new();

        let err = preflight_apple_container_with_runner(
            "container",
            "example.com/lgtm-codex:test",
            &auth_path,
            HostPlatform {
                os: "linux",
                arch: "x86_64",
                macos_major: None,
            },
            |program, args| {
                calls.push((program.to_string(), args.join(" ")));
                Ok(success_output())
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("requires macOS 26"));
        assert!(calls.is_empty());
    }

    #[test]
    fn apple_container_preflight_rejects_intel_macos_before_commands() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");
        let mut calls = Vec::new();

        let err = preflight_apple_container_with_runner(
            "container",
            "example.com/lgtm-codex:test",
            &auth_path,
            HostPlatform {
                os: "macos",
                arch: "x86_64",
                macos_major: Some(SUPPORTED_MACOS_MAJOR),
            },
            |program, args| {
                calls.push((program.to_string(), args.join(" ")));
                Ok(success_output())
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("requires Apple silicon"));
        assert!(calls.is_empty());
    }

    #[test]
    fn apple_container_preflight_rejects_old_macos_before_commands() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");
        let mut calls = Vec::new();

        let err = preflight_apple_container_with_runner(
            "container",
            "example.com/lgtm-codex:test",
            &auth_path,
            HostPlatform {
                os: "macos",
                arch: "aarch64",
                macos_major: Some(SUPPORTED_MACOS_MAJOR - 1),
            },
            |program, args| {
                calls.push((program.to_string(), args.join(" ")));
                Ok(success_output())
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("requires macOS 26"));
        assert!(calls.is_empty());
    }

    #[test]
    fn apple_container_preflight_uses_local_image_without_pulling() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");
        let mut calls = Vec::new();

        preflight_apple_container_with_runner(
            "container-test",
            "example.com/lgtm-codex:test",
            &auth_path,
            macos_26_arm64(),
            |program, args| {
                calls.push((program.to_string(), args.join(" ")));
                Ok(success_output())
            },
        )
        .expect("preflight");

        assert_eq!(
            calls,
            [
                ("container-test".to_string(), "system status".to_string()),
                (
                    "container-test".to_string(),
                    "image inspect example.com/lgtm-codex:test".to_string()
                ),
            ]
        );
    }

    #[test]
    fn apple_container_preflight_pulls_missing_image_before_launch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");
        let mut calls = Vec::new();

        preflight_apple_container_with_runner(
            "container-test",
            "example.com/lgtm-codex:test",
            &auth_path,
            macos_26_arm64(),
            |program, args| {
                calls.push((program.to_string(), args.join(" ")));
                if args == ["image", "inspect", "example.com/lgtm-codex:test"] {
                    Ok(command_output(1, "image not found"))
                } else {
                    Ok(success_output())
                }
            },
        )
        .expect("preflight");

        assert_eq!(
            calls,
            [
                ("container-test".to_string(), "system status".to_string()),
                (
                    "container-test".to_string(),
                    "image inspect example.com/lgtm-codex:test".to_string()
                ),
                (
                    "container-test".to_string(),
                    "image pull --progress none example.com/lgtm-codex:test".to_string()
                ),
            ]
        );
    }

    #[test]
    fn apple_container_preflight_can_use_fake_container_executable() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        let log_path = temp.path().join("container.log");
        let container_bin = temp.path().join("container-fake");
        fs::write(&auth_path, "{}").expect("auth");
        fs::write(
            &container_bin,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$1 $2 $3 $4 $5\" >> {}\ncase \"$1 $2\" in\n  'system status') exit 0 ;;\n  'image inspect') exit 1 ;;\n  'image pull') exit 0 ;;\n  *) exit 42 ;;\nesac\n",
                shell_quote(log_path.to_str().expect("utf-8 log path"))
            ),
        )
        .expect("fake container");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&container_bin, fs::Permissions::from_mode(0o755))
                .expect("fake container permissions");
        }

        preflight_apple_container_with_runner(
            container_bin.to_str().expect("utf-8 container path"),
            "example.com/lgtm-codex:test",
            &auth_path,
            macos_26_arm64(),
            run_command,
        )
        .expect("preflight");

        assert_eq!(
            fs::read_to_string(log_path).expect("container log"),
            "system status   \nimage inspect example.com/lgtm-codex:test  \nimage pull --progress none example.com/lgtm-codex:test\n"
        );
    }

    #[test]
    fn apple_container_preflight_reports_missing_container_cli_with_remediation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");

        let err = preflight_apple_container_with_runner(
            "/missing/container",
            "example.com/lgtm-codex:test",
            &auth_path,
            macos_26_arm64(),
            |_, _| {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "no such executable",
                ))
            },
        )
        .unwrap_err();
        let message = err.to_string();

        assert!(message.contains("configured container CLI `/missing/container`"));
        assert!(message.contains("/missing/container system start"));
        assert!(message.contains("--container-bin /path/to/container"));
    }

    #[test]
    fn apple_container_preflight_reports_stopped_services_with_remediation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");

        let err = preflight_apple_container_with_runner(
            "container",
            "example.com/lgtm-codex:test",
            &auth_path,
            macos_26_arm64(),
            |_, args| {
                if args == ["system", "status"] {
                    Ok(command_output(1, "apiserver is not running"))
                } else {
                    Ok(success_output())
                }
            },
        )
        .unwrap_err();
        let message = err.to_string();

        assert!(message.contains("container services are not running"));
        assert!(message.contains("container system start"));
        assert!(message.contains("apiserver is not running"));
    }

    #[test]
    fn apple_container_preflight_reports_unavailable_image_with_remediation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let auth_path = temp.path().join("auth.json");
        fs::write(&auth_path, "{}").expect("auth");

        let err = preflight_apple_container_with_runner(
            "container",
            "example.com/lgtm-codex:test",
            &auth_path,
            macos_26_arm64(),
            |_, args| {
                if args == ["image", "inspect", "example.com/lgtm-codex:test"] {
                    Ok(command_output(1, "image not found"))
                } else if args
                    == [
                        "image",
                        "pull",
                        "--progress",
                        "none",
                        "example.com/lgtm-codex:test",
                    ]
                {
                    Ok(command_output(1, "pull denied"))
                } else {
                    Ok(success_output())
                }
            },
        )
        .unwrap_err();
        let message = err.to_string();

        assert!(message.contains("sandbox image `example.com/lgtm-codex:test`"));
        assert!(
            message.contains("container image pull --progress none example.com/lgtm-codex:test")
        );
        assert!(message.contains("image not found"));
        assert!(message.contains("pull denied"));
    }
}
