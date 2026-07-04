use std::path::{Path, PathBuf};

pub(crate) const LOGS_DIR: &str = ".lgtm/logs";
pub(crate) const GATES_DIR: &str = ".lgtm/gates";
pub(crate) const SANDBOX_HOME_DIR: &str = ".lgtm/sandbox/home";
pub(crate) const SANDBOX_MISE_DIR: &str = ".lgtm/sandbox/mise";
pub(crate) const GITIGNORE_GENERATED_STATE: &str = ".lgtm/";

pub(crate) fn default_log_dir(root: &Path) -> PathBuf {
    root.join(LOGS_DIR)
}

pub(crate) fn default_gate_dir(root: &Path) -> PathBuf {
    root.join(GATES_DIR)
}

pub(crate) fn sandbox_home_dir(root: &Path) -> PathBuf {
    root.join(SANDBOX_HOME_DIR)
}

pub(crate) fn sandbox_mise_dir(root: &Path) -> PathBuf {
    root.join(SANDBOX_MISE_DIR)
}
