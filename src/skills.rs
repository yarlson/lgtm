use std::fs;
use std::path::Path;

use crate::Error;

const GITIGNORE_ENTRIES: &[&str] = &[".agents/skills/snap-*", ".codex-log/"];

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
    PHASE_IMPLEMENT => "snap-phase-implement", "../skills/snap-phase-implement/SKILL.md";
    PHASE_VALIDATE => "snap-phase-validate", "../skills/snap-phase-validate/SKILL.md";
    PHASE_REVIEW => "snap-phase-review", "../skills/snap-phase-review/SKILL.md";
    CONTEXT_MAP => "snap-context-map", "../skills/snap-context-map/SKILL.md";
    CLI_CONTROL => "snap-cli-control", "../skills/snap-cli-control/SKILL.md";
    UI_CONTROL => "snap-ui-control", "../skills/snap-ui-control/SKILL.md";
    TECHNICAL_SPIKE => "snap-technical-spike", "../skills/snap-technical-spike/SKILL.md";
    REFACTOR_PLAN => "snap-refactor-plan", "../skills/snap-refactor-plan/SKILL.md";
    PLAN_UPDATE => "snap-plan-update", "../skills/snap-plan-update/SKILL.md";
    SPEC_UPDATE => "snap-spec-update", "../skills/snap-spec-update/SKILL.md";
    SECURITY_REVIEW => "snap-security-review", "../skills/snap-security-review/SKILL.md";
    TEST_GAP_REVIEW => "snap-test-gap-review", "../skills/snap-test-gap-review/SKILL.md";
    DOCS_DRIFT_REVIEW => "snap-docs-drift-review", "../skills/snap-docs-drift-review/SKILL.md";
    ROLLOUT_REVIEW => "snap-rollout-review", "../skills/snap-rollout-review/SKILL.md";
    DEPENDENCY_REVIEW => "snap-dependency-review", "../skills/snap-dependency-review/SKILL.md";
    FINAL_REVIEW => "snap-final-review", "../skills/snap-final-review/SKILL.md";
}

pub fn install(root: &Path) -> Result<(), Error> {
    preflight(root)?;

    let skills_dir = root.join(".agents").join("skills");
    fs::create_dir_all(&skills_dir).map_err(|source| Error::io(&skills_dir, source))?;

    for skill in SKILLS {
        let skill_dir = skills_dir.join(skill.name);
        let skill_path = skill_dir.join("SKILL.md");
        fs::create_dir_all(&skill_dir).map_err(|source| Error::io(&skill_dir, source))?;
        fs::write(&skill_path, skill.body).map_err(|source| Error::io(&skill_path, source))?;
    }

    ensure_gitignore(root)
}

pub(crate) fn preflight(root: &Path) -> Result<(), Error> {
    let skills_dir = root.join(".agents").join("skills");

    for skill in SKILLS {
        let skill_path = skills_dir.join(skill.name).join("SKILL.md");
        if !skill_path.exists() {
            continue;
        }
        let existing =
            fs::read_to_string(&skill_path).map_err(|source| Error::io(&skill_path, source))?;
        if !is_managed_skill(skill.name, &existing) {
            return Err(Error::message(format!(
                "{} exists but is not managed by snap-rs",
                skill_path.display()
            )));
        }
    }

    Ok(())
}

fn ensure_gitignore(root: &Path) -> Result<(), Error> {
    let path = root.join(".gitignore");
    let mut content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(Error::io(&path, error)),
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
        fs::write(&path, content).map_err(|source| Error::io(&path, source))?;
    }
    Ok(())
}

fn is_managed_skill(expected_name: &str, body: &str) -> bool {
    frontmatter_value(body, "name").as_deref() == Some(expected_name)
        && frontmatter_value(body, "managed-by").as_deref() == Some("snap-rs")
}

fn frontmatter_value(body: &str, key: &str) -> Option<String> {
    let body = body.strip_prefix("---\n")?;
    let (frontmatter, _) = body.split_once("\n---")?;

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
    fn bundled_skills_have_snap_frontmatter() {
        for skill in SKILLS {
            assert!(is_managed_skill(skill.name, skill.body), "{}", skill.name);
        }
    }

    #[test]
    fn installs_managed_skills_and_gitignore_entry() {
        let temp = tempfile::tempdir().expect("tempdir");

        install(temp.path()).expect("install skills");
        install(temp.path()).expect("install skills twice");

        for skill in SKILLS {
            let skill_path = temp
                .path()
                .join(".agents")
                .join("skills")
                .join(skill.name)
                .join("SKILL.md");
            let body = fs::read_to_string(&skill_path).expect("skill body");
            assert!(is_managed_skill(skill.name, &body), "{}", skill.name);
            assert_eq!(body, skill.body, "{}", skill.name);
        }

        for expected in ["snap-phase-review", "snap-cli-control", "snap-ui-control"] {
            assert!(
                temp.path()
                    .join(".agents")
                    .join("skills")
                    .join(expected)
                    .join("SKILL.md")
                    .is_file(),
                "{expected}"
            );
        }

        let gitignore = fs::read_to_string(temp.path().join(".gitignore")).expect("gitignore");
        for entry in GITIGNORE_ENTRIES {
            assert_eq!(
                gitignore
                    .lines()
                    .filter(|line| line.trim() == *entry)
                    .count(),
                1,
                "{entry}"
            );
        }
    }

    #[test]
    fn preserves_non_snap_skills() {
        let temp = tempfile::tempdir().expect("tempdir");
        let custom_skill = temp
            .path()
            .join(".agents")
            .join("skills")
            .join("team-skill");
        fs::create_dir_all(&custom_skill).expect("create skill dir");
        let custom_path = custom_skill.join("SKILL.md");
        fs::write(&custom_path, "team owned").expect("write custom skill");

        install(temp.path()).expect("install skills");

        assert_eq!(
            fs::read_to_string(custom_path).expect("custom skill"),
            "team owned"
        );
    }

    #[test]
    fn refuses_to_overwrite_unmanaged_snap_skill() {
        let temp = tempfile::tempdir().expect("tempdir");
        let skill_dir = temp
            .path()
            .join(".agents")
            .join("skills")
            .join("snap-phase-implement");
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        let skill_path = skill_dir.join("SKILL.md");
        fs::write(&skill_path, "user owned").expect("write skill");

        let error = install(temp.path()).expect_err("should reject unmanaged snap skill");

        assert!(error.to_string().contains("is not managed by snap-rs"));
        assert_eq!(
            fs::read_to_string(skill_path).expect("skill body"),
            "user owned"
        );
    }

    #[test]
    fn refuses_unmanaged_snap_skill_before_writing_any_skill() {
        let temp = tempfile::tempdir().expect("tempdir");
        let managed_dir = temp
            .path()
            .join(".agents")
            .join("skills")
            .join(PHASE_IMPLEMENT);
        fs::create_dir_all(&managed_dir).expect("create managed skill dir");
        let managed_path = managed_dir.join("SKILL.md");
        fs::write(
            &managed_path,
            "---\nname: snap-phase-implement\nmanaged-by: snap-rs\n---\nold\n",
        )
        .expect("write managed skill");

        let unmanaged_dir = temp
            .path()
            .join(".agents")
            .join("skills")
            .join(PHASE_REVIEW);
        fs::create_dir_all(&unmanaged_dir).expect("create unmanaged skill dir");
        fs::write(unmanaged_dir.join("SKILL.md"), "user owned").expect("write unmanaged skill");

        let error = install(temp.path()).expect_err("should reject unmanaged snap skill");

        assert!(error.to_string().contains("is not managed by snap-rs"));
        assert_eq!(
            fs::read_to_string(managed_path).expect("managed skill"),
            "---\nname: snap-phase-implement\nmanaged-by: snap-rs\n---\nold\n"
        );
    }

    #[test]
    fn marker_text_outside_frontmatter_does_not_grant_ownership() {
        let body = "team note\nmanaged-by: snap-rs\n";

        assert!(!is_managed_skill("snap-phase-implement", body));
    }

    #[test]
    fn mismatched_frontmatter_name_does_not_grant_ownership() {
        let body = "---\nname: snap-other\nmanaged-by: snap-rs\n---\n";

        assert!(!is_managed_skill("snap-phase-implement", body));
    }
}
