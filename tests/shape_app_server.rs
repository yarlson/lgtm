use std::{fs, process::Command};

#[test]
fn shape_brief_intake_does_not_require_tty() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("shape")
        .arg("brief idea")
        .arg("--root")
        .arg(&repo)
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("lgtm shape runtime is not implemented yet"),
        "{stderr}"
    );
    assert!(!stderr.contains("requires interactive stdin and stdout"));
}

#[test]
fn shape_file_brief_intake_does_not_require_tty() {
    let temp = tempfile::tempdir().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    fs::write(repo.join("brief.md"), "brief idea").expect("brief");

    let output = Command::new(env!("CARGO_BIN_EXE_lgtm"))
        .arg("shape")
        .arg("--brief-file")
        .arg("brief.md")
        .arg("--root")
        .arg(&repo)
        .output()
        .expect("run lgtm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("lgtm shape runtime is not implemented yet"),
        "{stderr}"
    );
    assert!(!stderr.contains("requires interactive stdin and stdout"));
}
