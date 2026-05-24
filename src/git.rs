use anyhow::{Context, Result, bail};
use dialoguer::Confirm;
use std::path::Path;
use std::process::Command;

pub fn ensure_initialized(root: &Path) -> Result<()> {
    ensure_initialized_with(root, confirm_initialize)
}

fn ensure_initialized_with(root: &Path, confirm: impl FnOnce(&Path) -> Result<bool>) -> Result<()> {
    if is_git_root(root)? {
        return Ok(());
    }

    if !confirm(root)? {
        bail!(
            "{} is not a git repository; initialize git before running lgtm",
            root.display()
        );
    }

    run_git(root, &["init"])?;
    run_git(root, &["branch", "-M", "main"])
}

fn confirm_initialize(root: &Path) -> Result<bool> {
    Confirm::new()
        .with_prompt(format!(
            "{} is not a git repository. Initialize git and rename the branch to main?",
            root.display()
        ))
        .default(true)
        .interact()
        .context("failed to read git preflight answer")
}

fn is_git_root(root: &Path) -> Result<bool> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git rev-parse")?;

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
        .with_context(|| format!("failed to canonicalize {}", root.display()))?;
    let top_level = Path::new(top_level)
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {top_level}"))?;
    Ok(root == top_level)
}

fn run_git(root: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .context("failed to run git")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("git {} failed: {}", args.join(" "), stderr.trim())
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
}
