use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::plan_contract;

pub(super) fn parse_final_plan_marker(response: &str) -> Result<Option<PathBuf>> {
    for line in response.lines() {
        let line = line.trim();
        let Some(path) = line.strip_prefix("PLAN_PATH:") else {
            continue;
        };
        let path = path.trim();
        if path.is_empty() {
            bail!("shape final marker PLAN_PATH is empty")
        }
        return Ok(Some(PathBuf::from(path)));
    }

    Ok(None)
}

pub(super) fn validate_reported_plan_path(
    root: &Path,
    configured_plan_path: &Path,
    resolved_plan_path: &Path,
    reported_plan_path: &Path,
) -> Result<()> {
    let reported_path = if reported_plan_path.is_absolute() {
        if !configured_plan_path.is_absolute() {
            bail!(
                "shape final marker reported absolute plan path {} but --plan-path was not absolute",
                reported_plan_path.display()
            )
        }
        reported_plan_path.to_path_buf()
    } else {
        root.join(reported_plan_path)
    };

    let reported_canonical = canonicalize_plan_file(&reported_path)?;
    let expected_canonical = canonicalize_plan_file(resolved_plan_path)?;
    if reported_canonical != expected_canonical {
        bail!(
            "shape final marker reported {}, but configured plan path is {}",
            reported_plan_path.display(),
            resolved_plan_path.display()
        )
    }

    if !configured_plan_path.is_absolute() {
        let root_canonical = fs::canonicalize(root)
            .with_context(|| format!("failed to resolve target root {}", root.display()))?;
        if !reported_canonical.starts_with(&root_canonical) {
            bail!(
                "shape final marker reported plan path outside target root: {}",
                reported_plan_path.display()
            )
        }
    }

    Ok(())
}

pub(super) fn validate_final_plan_contract(plan_path: &Path) -> Result<()> {
    plan_contract::validate_plan_file(plan_path)
}

fn canonicalize_plan_file(path: &Path) -> Result<PathBuf> {
    if !path.is_file() {
        bail!(
            "reported shape plan path does not exist: {}",
            path.display()
        )
    }
    fs::canonicalize(path)
        .with_context(|| format!("failed to resolve shape plan path {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SHAPE_PLAN: &str = "\
# Plan

## Decisions

- Ship the smallest viable implementation.

## Non-Goals

- Do not broaden scope.

## Open Risks

- Keep validation explicit.

## Loopholes To Close

- Confirm runtime behavior before implementation.

## Phase 1 - Test

Goal:
Ship.

Deliverables:
- Shipped change.

Dependencies:
- None.

Unresolved decisions:
- None.

Steps:
- Do it.

Validation:
- Check it.
";

    #[test]
    fn final_plan_marker_parser_accepts_marker_line() {
        let marker = parse_final_plan_marker("Done.\nPLAN_PATH: docs/PLAN.md\n").expect("marker");

        assert_eq!(marker, Some(PathBuf::from("docs/PLAN.md")));
    }

    #[test]
    fn final_plan_marker_parser_rejects_empty_marker() {
        let error = parse_final_plan_marker("PLAN_PATH: ").expect_err("empty marker");

        assert!(error.to_string().contains("PLAN_PATH is empty"));
    }

    #[test]
    fn final_plan_marker_parser_returns_none_without_marker() {
        let marker = parse_final_plan_marker("Ask next question").expect("marker");

        assert_eq!(marker, None);
    }

    #[test]
    fn validates_reported_plan_path_under_target_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        fs::write(root.join("PLAN.md"), VALID_SHAPE_PLAN).expect("plan");

        validate_reported_plan_path(
            &root,
            Path::new("PLAN.md"),
            &root.join("PLAN.md"),
            Path::new("PLAN.md"),
        )
        .expect("path");
    }

    #[test]
    fn rejects_missing_reported_plan_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");

        let error = validate_reported_plan_path(
            &root,
            Path::new("PLAN.md"),
            &root.join("PLAN.md"),
            Path::new("PLAN.md"),
        )
        .expect_err("missing plan");

        assert!(
            error
                .to_string()
                .contains("reported shape plan path does not exist")
        );
    }

    #[test]
    fn rejects_relative_reported_plan_path_outside_target_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let outside_plan = temp.path().join("PLAN.md");
        fs::write(&outside_plan, "# Plan\n").expect("plan");

        let error = validate_reported_plan_path(
            &root,
            Path::new("../PLAN.md"),
            &outside_plan,
            Path::new("../PLAN.md"),
        )
        .expect_err("outside root");

        assert!(error.to_string().contains("outside target root"));
    }

    #[test]
    fn rejects_absolute_reported_plan_path_without_absolute_arg() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        fs::create_dir(&root).expect("repo");
        let plan = root.join("PLAN.md");
        fs::write(&plan, "# Plan\n").expect("plan");

        let error = validate_reported_plan_path(&root, Path::new("PLAN.md"), &plan, &plan)
            .expect_err("absolute marker");

        assert!(error.to_string().contains("--plan-path was not absolute"));
    }

    #[test]
    fn accepts_absolute_reported_plan_path_when_configured_absolute() {
        let temp = tempfile::tempdir().expect("tempdir");
        let plan = temp.path().join("PLAN.md");
        fs::write(&plan, "# Plan\n").expect("plan");

        validate_reported_plan_path(temp.path(), &plan, &plan, &plan).expect("path");
    }
}
