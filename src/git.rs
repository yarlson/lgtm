use std::path::Path;
use std::process::Command;

use dialoguer::Confirm;

use crate::Error;

pub fn ensure_initialized(root: &Path) -> Result<(), Error> {
    ensure_initialized_with(root, confirm_initialize)
}

fn ensure_initialized_with(
    root: &Path,
    confirm: impl FnOnce(&Path) -> Result<bool, Error>,
) -> Result<(), Error> {
    if is_git_root(root)? {
        return Ok(());
    }

    if !confirm(root)? {
        return Err(Error::message(format!(
            "{} is not a git repository; initialize git before running lgtm",
            root.display()
        )));
    }

    run_git(root, &["init"])?;
    run_git(root, &["branch", "-M", "main"])
}

fn confirm_initialize(root: &Path) -> Result<bool, Error> {
    Confirm::new()
        .with_prompt(format!(
            "{} is not a git repository. Initialize git and rename the branch to main?",
            root.display()
        ))
        .default(true)
        .interact()
        .map_err(|source| Error::message(format!("failed to read git preflight answer: {source}")))
}

fn is_git_root(root: &Path) -> Result<bool, Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|source| Error::io("git", source))?;

    if !output.status.success() {
        return Ok(false);
    }

    let top_level = String::from_utf8_lossy(&output.stdout);
    let top_level = top_level.trim();
    if top_level.is_empty() {
        return Ok(false);
    }

    let root = root
        .canonicalize()
        .map_err(|source| Error::io(root, source))?;
    let top_level = Path::new(top_level)
        .canonicalize()
        .map_err(|source| Error::io(top_level, source))?;
    Ok(root == top_level)
}

fn run_git(root: &Path, args: &[&str]) -> Result<(), Error> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|source| Error::io("git", source))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::message(format!(
        "git {} failed: {}",
        args.join(" "),
        stderr.trim()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_git_repo_and_renames_branch_when_confirmed() {
        let temp = tempfile::tempdir().expect("tempdir");

        ensure_initialized_with(temp.path(), |_| Ok(true)).expect("initialize git");

        assert!(temp.path().join(".git").is_dir());
        let head = Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["symbolic-ref", "--short", "HEAD"])
            .output()
            .expect("git head");
        assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "main");
    }

    #[test]
    fn aborts_when_user_declines_git_init() {
        let temp = tempfile::tempdir().expect("tempdir");

        let error = ensure_initialized_with(temp.path(), |_| Ok(false)).expect_err("decline");

        assert!(error.to_string().contains("is not a git repository"));
        assert!(!temp.path().join(".git").exists());
    }

    #[test]
    fn accepts_existing_git_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        run_git(temp.path(), &["init"]).expect("git init");
        run_git(temp.path(), &["branch", "-M", "main"]).expect("branch main");

        ensure_initialized_with(temp.path(), |_| panic!("should not prompt")).expect("preflight");
    }
}
