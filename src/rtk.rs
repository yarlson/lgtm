use std::{env, ffi::OsStr, fs, path::Path};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(crate) const DEVELOPER_INSTRUCTIONS: &str = include_str!("../assets/RTK.md");

pub(crate) fn developer_instructions_suffix() -> Option<&'static str> {
    let path = env::var_os("PATH");
    developer_instructions_suffix_for_path(path.as_deref())
}

fn developer_instructions_suffix_for_path(path: Option<&OsStr>) -> Option<&'static str> {
    path.is_some_and(command_path_contains_rtk)
        .then_some(DEVELOPER_INSTRUCTIONS)
}

fn command_path_contains_rtk(path: &OsStr) -> bool {
    env::split_paths(path)
        .filter(|directory| !directory.as_os_str().is_empty())
        .any(|directory| rtk_path_exists(&directory))
}

fn rtk_path_exists(directory: &Path) -> bool {
    let candidate = directory.join("rtk");
    if command_file_exists(&candidate) {
        return true;
    }

    if cfg!(windows) {
        return command_file_exists(&candidate.with_extension("exe"));
    }

    false
}

fn command_file_exists(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_no_suffix_without_path() {
        assert!(developer_instructions_suffix_for_path(None).is_none());
    }

    #[test]
    fn returns_bundled_instructions_when_rtk_is_on_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("rtk"), "").expect("rtk");
        make_executable(&temp.path().join("rtk"));
        let path = env::join_paths([temp.path()]).expect("path");

        let suffix = developer_instructions_suffix_for_path(Some(&path)).expect("suffix");

        assert!(!suffix.trim().is_empty());
    }

    #[test]
    fn ignores_non_executable_rtk_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("rtk"), "").expect("rtk");
        let path = env::join_paths([temp.path()]).expect("path");

        assert!(developer_instructions_suffix_for_path(Some(&path)).is_none());
    }

    #[test]
    fn ignores_missing_rtk() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = env::join_paths([temp.path()]).expect("path");

        assert!(developer_instructions_suffix_for_path(Some(&path)).is_none());
    }

    fn make_executable(path: &Path) {
        #[cfg(unix)]
        {
            let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions).expect("chmod");
        }
    }
}
