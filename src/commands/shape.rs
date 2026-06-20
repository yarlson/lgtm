use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::cli::ShapeArgs;

pub fn run(args: ShapeArgs) -> Result<()> {
    let _brief = read_brief(&args)?;

    bail!("lgtm shape runtime is not implemented yet")
}

fn read_brief(args: &ShapeArgs) -> Result<String> {
    let content = match (&args.brief, &args.brief_file) {
        (Some(_), Some(_)) => {
            bail!(
                "lgtm shape accepts exactly one brief source; pass either BRIEF or --brief-file PATH, not both"
            )
        }
        (None, None) => {
            bail!("lgtm shape requires a brief source; pass BRIEF or --brief-file PATH")
        }
        (Some(brief), None) => brief.clone(),
        (None, Some(brief_file)) => {
            let path = if brief_file.is_absolute() {
                brief_file.to_path_buf()
            } else {
                target_root(args.root.as_deref())?.join(brief_file)
            };
            fs::read_to_string(&path)
                .with_context(|| format!("failed to read brief file {}", path.display()))?
        }
    };

    let brief = content.trim();
    if brief.is_empty() {
        bail!("lgtm shape brief cannot be empty; provide non-whitespace content")
    }

    Ok(brief.to_string())
}

fn target_root(root: Option<&Path>) -> Result<PathBuf> {
    match root {
        Some(path) if path.is_absolute() => Ok(path.to_path_buf()),
        Some(path) => Ok(std::env::current_dir()
            .context("failed to read current directory")?
            .join(path)),
        None => std::env::current_dir().context("failed to read current directory"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{ExecutionArgs, ShapeArgs, StreamMode};

    fn shape_args() -> ShapeArgs {
        ShapeArgs {
            brief: None,
            brief_file: None,
            root: None,
            plan_path: "PLAN.md".into(),
            codex_bin: "codex".to_string(),
            execution: ExecutionArgs::default(),
            stream_mode: StreamMode::Pretty,
            log_dir: None,
            run_stamp: None,
            max_rounds: 12,
        }
    }

    #[test]
    fn accepts_string_brief() {
        let mut args = shape_args();
        args.brief = Some("  ship smaller phases  ".to_string());

        assert_eq!(read_brief(&args).expect("brief"), "ship smaller phases");
    }

    #[test]
    fn accepts_file_brief_relative_to_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        fs::create_dir(root.join("docs")).expect("docs");
        fs::write(root.join("docs/brief.md"), "\nshape this\n").expect("brief");
        let mut args = shape_args();
        args.root = Some(root);
        args.brief_file = Some("docs/brief.md".into());

        assert_eq!(read_brief(&args).expect("brief"), "shape this");
    }

    #[test]
    fn accepts_absolute_file_brief() {
        let temp = tempfile::tempdir().expect("tempdir");
        let brief_file = temp.path().join("brief.md");
        fs::write(&brief_file, "shape absolute").expect("brief");
        let mut args = shape_args();
        args.root = Some(temp.path().join("other-root"));
        args.brief_file = Some(brief_file);

        assert_eq!(read_brief(&args).expect("brief"), "shape absolute");
    }

    #[test]
    fn rejects_missing_brief_source() {
        let error = read_brief(&shape_args()).expect_err("missing source");

        assert!(
            error
                .to_string()
                .contains("requires a brief source; pass BRIEF or --brief-file PATH")
        );
    }

    #[test]
    fn rejects_both_brief_sources() {
        let mut args = shape_args();
        args.brief = Some("brief".to_string());
        args.brief_file = Some("docs/brief.md".into());

        let error = read_brief(&args).expect_err("both sources");

        assert!(
            error
                .to_string()
                .contains("pass either BRIEF or --brief-file PATH, not both")
        );
    }

    #[test]
    fn rejects_missing_file_brief() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut args = shape_args();
        args.root = Some(temp.path().to_path_buf());
        args.brief_file = Some("missing.md".into());

        let error = read_brief(&args).expect_err("missing file");

        assert!(error.to_string().contains("failed to read brief file"));
        assert!(error.to_string().contains("missing.md"));
    }

    #[test]
    fn rejects_empty_brief() {
        let mut args = shape_args();
        args.brief = Some(" \n\t ".to_string());

        let error = read_brief(&args).expect_err("empty brief");

        assert!(
            error
                .to_string()
                .contains("brief cannot be empty; provide non-whitespace content")
        );
    }

    #[test]
    fn rejects_empty_file_brief() {
        let temp = tempfile::tempdir().expect("tempdir");
        let brief_file = temp.path().join("brief.md");
        fs::write(&brief_file, " \n\t ").expect("brief");
        let mut args = shape_args();
        args.brief_file = Some(brief_file);

        let error = read_brief(&args).expect_err("empty brief");

        assert!(
            error
                .to_string()
                .contains("brief cannot be empty; provide non-whitespace content")
        );
    }
}
