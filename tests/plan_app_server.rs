use std::{fs, process::Command};

#[test]
fn plan_mode_rejects_non_tty_before_repo_preflight() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");

    let unmanaged_skill = repo
        .join(".agents")
        .join("skills")
        .join("lgtm-phase-implement");
    fs::create_dir_all(&unmanaged_skill).expect("skill dir");
    fs::write(unmanaged_skill.join("SKILL.md"), "project owned").expect("skill");

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("plan")
        .arg("--root")
        .arg(&repo)
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires interactive stdin and stdout"));
    assert!(!repo.join(".git").exists());
}
