use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

use crate::paths;

const GITIGNORE_ENTRIES: &[&str] = &[".agents/skills/lgtm-*", paths::GITIGNORE_GENERATED_STATE];

struct Skill {
    name: &'static str,
    body: &'static str,
}

macro_rules! skill_registry {
    ($($constant:ident => $name:literal, $path:literal;)+) => {
        $(pub(crate) const $constant: &str = $name;)+

        const SKILLS: &[Skill] = &[
            $(Skill {
                name: $constant,
                body: include_str!($path),
            },)+
        ];
    };
}

skill_registry! {
    PHASE_IMPLEMENT => "lgtm-phase-implement", "../skills/lgtm-phase-implement/SKILL.md";
    PHASE_VALIDATE => "lgtm-phase-validate", "../skills/lgtm-phase-validate/SKILL.md";
    PHASE_REVIEW => "lgtm-phase-review", "../skills/lgtm-phase-review/SKILL.md";
    PHASE_COMMIT => "lgtm-phase-commit", "../skills/lgtm-phase-commit/SKILL.md";
    CONTEXT_MAP => "lgtm-context-map", "../skills/lgtm-context-map/SKILL.md";
    CLI_CONTROL => "lgtm-cli-control", "../skills/lgtm-cli-control/SKILL.md";
    UI_CONTROL => "lgtm-ui-control", "../skills/lgtm-ui-control/SKILL.md";
    TECHNICAL_SPIKE => "lgtm-technical-spike", "../skills/lgtm-technical-spike/SKILL.md";
    REFACTOR_PLAN => "lgtm-refactor-plan", "../skills/lgtm-refactor-plan/SKILL.md";
    PLAN_UPDATE => "lgtm-plan-update", "../skills/lgtm-plan-update/SKILL.md";
    SPEC_UPDATE => "lgtm-spec-update", "../skills/lgtm-spec-update/SKILL.md";
    SECURITY_REVIEW => "lgtm-security-review", "../skills/lgtm-security-review/SKILL.md";
    PLAN_CREATE => "lgtm-plan-create", "../skills/lgtm-plan-create/SKILL.md";
    TEST_GAP_REVIEW => "lgtm-test-gap-review", "../skills/lgtm-test-gap-review/SKILL.md";
    DOCS_DRIFT_REVIEW => "lgtm-docs-drift-review", "../skills/lgtm-docs-drift-review/SKILL.md";
    ROLLOUT_REVIEW => "lgtm-rollout-review", "../skills/lgtm-rollout-review/SKILL.md";
    DEPENDENCY_REVIEW => "lgtm-dependency-review", "../skills/lgtm-dependency-review/SKILL.md";
    FINAL_REVIEW => "lgtm-final-review", "../skills/lgtm-final-review/SKILL.md";
}

pub fn install(root: &Path) -> Result<()> {
    preflight(root)?;

    let skills_dir = root.join(".agents").join("skills");
    fs::create_dir_all(&skills_dir)
        .with_context(|| format!("failed to create {}", skills_dir.display()))?;

    for skill in SKILLS {
        let skill_dir = skills_dir.join(skill.name);
        let skill_path = skill_dir.join("SKILL.md");
        fs::create_dir_all(&skill_dir)
            .with_context(|| format!("failed to create {}", skill_dir.display()))?;
        fs::write(&skill_path, skill.body)
            .with_context(|| format!("failed to write {}", skill_path.display()))?;
    }

    ensure_gitignore(root)
}

pub fn preflight(root: &Path) -> Result<()> {
    let skills_dir = root.join(".agents").join("skills");
    reject_unmanaged_lgtm_skills(&skills_dir)?;

    for skill in SKILLS {
        let skill_path = skills_dir.join(skill.name).join("SKILL.md");
        if !skill_path.exists() {
            continue;
        }
        let existing = fs::read_to_string(&skill_path)
            .with_context(|| format!("failed to read {}", skill_path.display()))?;
        if !is_managed_skill(skill.name, &existing) {
            bail!("{} exists but is not managed by lgtm", skill_path.display());
        }
    }

    Ok(())
}

fn reject_unmanaged_lgtm_skills(skills_dir: &Path) -> Result<()> {
    let entries = match fs::read_dir(skills_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", skills_dir.display()));
        }
    };

    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", skills_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with("lgtm-") {
            continue;
        }

        let skill_path = entry.path().join("SKILL.md");
        let existing = match fs::read_to_string(&skill_path) {
            Ok(existing) => existing,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                bail!("{} exists but is not managed by lgtm", skill_path.display());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {}", skill_path.display()));
            }
        };
        if !is_managed_skill(name, &existing) {
            bail!("{} exists but is not managed by lgtm", skill_path.display());
        }
    }

    Ok(())
}

fn ensure_gitignore(root: &Path) -> Result<()> {
    let path = root.join(".gitignore");
    let mut content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };

    let mut changed = false;
    for entry in GITIGNORE_ENTRIES {
        if content.lines().any(|line| line.trim() == *entry) {
            continue;
        }
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(entry);
        content.push('\n');
        changed = true;
    }

    if changed {
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn is_managed_skill(expected_name: &str, body: &str) -> bool {
    let Some(frontmatter) = frontmatter(body) else {
        return false;
    };

    frontmatter_value(&frontmatter, "name").as_deref() == Some(expected_name)
        && frontmatter_value(&frontmatter, "managed-by").as_deref() == Some("lgtm")
}

fn frontmatter(body: &str) -> Option<String> {
    let mut lines = body.lines();
    if lines.next()? != "---" {
        return None;
    }

    let mut frontmatter = Vec::new();
    for line in lines {
        if line == "---" {
            return Some(frontmatter.join("\n"));
        }
        frontmatter.push(line);
    }

    None
}

fn frontmatter_value(frontmatter: &str, key: &str) -> Option<String> {
    frontmatter.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim() == key {
            Some(value.trim().trim_matches('"').to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_skills_have_lgtm_frontmatter() {
        for skill in SKILLS {
            assert!(is_managed_skill(skill.name, skill.body), "{}", skill.name);
        }
    }

    #[test]
    fn phase_review_skill_requires_strict_fixing_review() {
        let body = SKILLS
            .iter()
            .find(|skill| skill.name == PHASE_REVIEW)
            .expect("phase review skill")
            .body;

        assert!(body.contains("code-judo"));
        assert!(body.contains("Fix every safe, phase-scoped finding"));
        assert!(body.contains("$lgtm-refactor-plan"));
        assert!(body.contains("1000 lines"));
        assert!(body.contains("No approve just because behavior seems correct"));
        assert!(body.contains("Non-Negotiable Review Rules"));
        assert!(body.contains("Do not soften major maintainability issues"));
        assert!(body.contains("Approval Bar"));
        assert!(body.contains("no obvious missed opportunity"));
        assert!(body.contains("Commit pass owns committing"));
    }

    #[test]
    fn phase_commit_skill_rejects_commit_message_inventories() {
        let body = SKILLS
            .iter()
            .find(|skill| skill.name == PHASE_COMMIT)
            .expect("phase commit skill")
            .body;

        assert!(body.contains("Prefer subject-only"));
        assert!(body.contains("Never: changed-file list"));
        assert!(body.contains("verification section"));
        assert!(!body.contains("Key changes"));
    }

    #[test]
    fn malformed_frontmatter_does_not_authorize_overwrite() {
        let body = "\
---
name: lgtm-phase-implement
managed-by: lgtm
---not-a-frontmatter-close
";

        assert!(!is_managed_skill("lgtm-phase-implement", body));
    }

    #[test]
    fn install_ignores_lgtm_generated_state() {
        let temp = tempfile::tempdir().expect("tempdir");

        install(temp.path()).expect("install");

        let gitignore = fs::read_to_string(temp.path().join(".gitignore")).expect("gitignore");
        assert!(gitignore.lines().any(|line| line == ".lgtm/"));
        assert!(!gitignore.lines().any(|line| line == ".codex-log/"));
    }

    #[test]
    fn install_leaves_existing_codex_log_ignore_entry() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join(".gitignore"), ".codex-log/\n").expect("gitignore");

        install(temp.path()).expect("install");

        let gitignore = fs::read_to_string(temp.path().join(".gitignore")).expect("gitignore");
        assert!(gitignore.lines().any(|line| line == ".codex-log/"));
        assert!(gitignore.lines().any(|line| line == ".lgtm/"));
    }

    #[test]
    fn refuses_to_overwrite_unmanaged_lgtm_skill() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp
            .path()
            .join(".agents")
            .join("skills")
            .join("lgtm-phase-implement");
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        let skill_path = skill_dir.join("SKILL.md");
        fs::write(&skill_path, "user owned").expect("write skill");

        let error = install(temp.path()).expect_err("should reject unmanaged lgtm skill");

        assert!(error.to_string().contains("is not managed by lgtm"));
        assert_eq!(
            fs::read_to_string(skill_path).expect("skill body"),
            "user owned"
        );
    }
}
