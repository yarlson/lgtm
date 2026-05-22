use std::fs;
use std::path::Path;

use crate::Error;

const MANAGED_MARKER: &str = "managed-by: snap-rs";
const GITIGNORE_ENTRY: &str = ".agents/skills/snap-*";

struct Skill {
    name: &'static str,
    body: &'static str,
}

pub fn install(root: &Path) -> Result<(), Error> {
    let skills_dir = root.join(".agents").join("skills");
    fs::create_dir_all(&skills_dir).map_err(|source| Error::io(&skills_dir, source))?;

    for skill in SKILLS {
        let skill_dir = skills_dir.join(skill.name);
        let skill_path = skill_dir.join("SKILL.md");
        if skill_path.exists() {
            let existing =
                fs::read_to_string(&skill_path).map_err(|source| Error::io(&skill_path, source))?;
            if !existing.contains(MANAGED_MARKER) {
                return Err(Error::message(format!(
                    "{} exists but is not managed by snap-rs",
                    skill_path.display()
                )));
            }
        }
        fs::create_dir_all(&skill_dir).map_err(|source| Error::io(&skill_dir, source))?;
        fs::write(&skill_path, skill.body).map_err(|source| Error::io(&skill_path, source))?;
    }

    ensure_gitignore(root)
}

fn ensure_gitignore(root: &Path) -> Result<(), Error> {
    let path = root.join(".gitignore");
    let mut content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(Error::io(&path, error)),
    };

    if content.lines().any(|line| line.trim() == GITIGNORE_ENTRY) {
        return Ok(());
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(GITIGNORE_ENTRY);
    content.push('\n');

    fs::write(&path, content).map_err(|source| Error::io(&path, source))
}

const SKILLS: &[Skill] = &[
    Skill {
        name: "snap-phase-implement",
        body: r#"---
name: snap-phase-implement
description: "snap-rs implementation pass for exactly one PLAN.md phase. Use when snap-rs asks Codex to implement a selected phase. Reads PLAN.md, AGENTS.md, and DESIGN.md, maps relevant files, implements only the selected phase, keeps changes surgical, and verifies before finishing."
managed-by: snap-rs
---

# snap-rs Phase Implementation

You are implementing exactly one selected phase from a repo-local `PLAN.md`.

## Inputs

snap-rs will provide:

- the target phase heading, for example `## Phase 4 - Path And Environment Resolution`
- the path to `PLAN.md`
- the path to `AGENTS.md`
- the path to `DESIGN.md`

Treat these files as authoritative.

## Workflow

1. Open `AGENTS.md`, `DESIGN.md`, and `PLAN.md`.
2. Locate the exact selected phase heading in `PLAN.md`.
3. Read only the selected phase plus any directly referenced sections needed to understand it.
4. Map the files relevant to this phase before editing.
5. Inspect current implementation patterns in those files and nearby modules.
6. State assumptions only when they affect implementation.
7. Implement only the selected phase.
8. Do not skip ahead into later phases unless the selected phase explicitly requires a small prerequisite.
9. Keep the diff surgical and consistent with the existing codebase.
10. Run the checks required by `AGENTS.md` and the selected phase.
11. Fix failures within selected-phase scope.
12. Before finishing, confirm that the selected phase is complete end to end.

## Scope Rules

Do not add unrelated features, commands, flags, workflows, release automation, CI, configuration, abstractions, or documentation.

Update `DESIGN.md` only when implementation exposes a real product-contract gap.

Update `PLAN.md` only when the selected phase needs a corrected implementation order or validation gate.

Do not commit or push unless explicitly requested.

## Completion Criteria

The implementation pass is complete only when:

- the selected phase's Goal and Steps are satisfied
- required validation commands were run or a blocker is clearly reported
- touched code follows local patterns
- no later-phase work was introduced
- no unrelated cleanup was included
"#,
    },
    Skill {
        name: "snap-phase-validate",
        body: r#"---
name: snap-phase-validate
description: "snap-rs validation pass for exactly one PLAN.md phase. Use when snap-rs asks Codex to validate an implemented phase. Independently re-reads the selected phase, compares implementation to Goal, Steps, Validation, and Web validation sections, fixes scoped gaps, and verifies concrete checks."
managed-by: snap-rs
---

# snap-rs Phase Validation

You are validating exactly one selected phase from `PLAN.md`.

## Inputs

snap-rs will provide:

- the selected phase heading
- the path to `PLAN.md`
- the path to `AGENTS.md`
- the path to `DESIGN.md`

Treat validation as an independent review, not a continuation of implementation assumptions.

## Workflow

1. Re-open `AGENTS.md`, `DESIGN.md`, and `PLAN.md`.
2. Locate the exact selected phase heading.
3. Re-read the selected phase's Goal, Steps, Validation, and Web validation sections.
4. Inspect files touched by the implementation and surrounding modules.
5. Compare current behavior against the phase contract.
6. Look for:
   - missing behavior
   - incomplete edge cases
   - unsafe broad changes
   - weak or missing tests
   - stale docs or product-contract drift
   - security-sensitive surfaces introduced by the change
   - required checks that were skipped
7. Fix only gaps needed to complete the selected phase.
8. Strengthen tests or verification when existing checks do not prove the phase works.
9. Run required checks again after fixes.
10. Review the final diff before finishing.

## Validation Standard

Do not accept a phase because code exists. Accept it only when behavior is verified against the phase contract.

If the phase cannot be validated because a tool, service, credential, fixture, or environment is missing, report the blocker explicitly and explain what remains unverified.

## Completion Criteria

Validation is complete only when:

- the selected phase is implemented fully and correctly
- concrete checks were run or blockers were reported
- any fixes stayed within selected-phase scope
- no later-phase work was added
- final diff was reviewed
"#,
    },
    Skill {
        name: "snap-context-map",
        body: r#"---
name: snap-context-map
description: "snap-rs context discovery skill. Use before implementation or validation to identify files, docs, commands, risks, unknowns, and local patterns relevant to the selected PLAN.md phase."
managed-by: snap-rs
---

# snap-rs Context Map

Use this before editing or validating a selected phase.

The goal is to gather enough local context to work safely without reading the whole repository.

## Workflow

1. Read the selected `PLAN.md` phase.
2. Read `AGENTS.md` for repo instructions.
3. Read `DESIGN.md` for product or architecture constraints.
4. Search for files, modules, tests, commands, docs, and config relevant to the selected phase.
5. Inspect nearby code patterns before deciding how to implement or validate.
6. Identify unknowns that affect implementation correctness.
7. Resolve discoverable unknowns through repo-local files, config, tests, or installed tool versions.
8. Use official docs only when local evidence is insufficient for unfamiliar or version-sensitive behavior.

## Output To Keep In Working Memory

Before proceeding, know:

- relevant source files
- relevant tests
- relevant commands
- local conventions to follow
- likely risk areas
- implementation assumptions, if any
- validation evidence needed

## Guardrails

Do not turn context mapping into broad documentation work.

Do not inspect generated output, build artifacts, vendored dependencies, or unrelated modules unless the selected phase requires it.

Do not ask the user for file locations that can be discovered locally.

## Completion Criteria

Context mapping is complete when you can explain:

- what files you need to touch
- what files you need to verify
- what repo conventions constrain the change
- what risks or unknowns remain
"#,
    },
    Skill {
        name: "snap-technical-spike",
        body: r#"---
name: snap-technical-spike
description: "snap-rs bounded technical spike skill. Use when a selected phase depends on unknown, unfamiliar, or version-sensitive framework, library, tool, runtime, or platform behavior. Produces implementation-relevant conclusions without drifting into broad research."
managed-by: snap-rs
---

# snap-rs Technical Spike

Use this only when implementation or validation depends on unknown technical behavior.

Examples:

- unfamiliar framework APIs
- version-sensitive Rust, Cargo, Git, dependency, or test-runner behavior
- unclear build or runtime constraints
- a library behavior that affects implementation shape
- a tool command needed for validation

## Workflow

1. State the exact technical question.
2. Check repo-local evidence first:
   - manifests
   - lockfiles
   - config files
   - existing code
   - tests
   - installed versions
   - local help output
3. If local evidence is insufficient, consult current official docs for the specific tool or library.
4. Record only conclusions that affect the selected phase.
5. Convert findings into implementation or validation implications.
6. Stop once the phase can be implemented or validated safely.

## Output Shape

Use this concise structure in your reasoning or final report:

```md
## Technical Spike Result

Question: ...
Local evidence: ...
External evidence, if used: ...
Conclusion: ...
Impact on this phase: ...
Remaining uncertainty: ...
```

## Guardrails

Do not research adjacent features.

Do not upgrade dependencies unless the selected phase explicitly requires it.

Do not add abstractions to hide uncertainty.

Do not cite unofficial sources when official docs are available.

## Completion Criteria

The spike is complete when the selected phase has a clear implementation or validation path.
"#,
    },
    Skill {
        name: "snap-refactor-plan",
        body: r#"---
name: snap-refactor-plan
description: "snap-rs refactor planning skill. Use when the selected PLAN.md phase is a refactor, cleanup, migration, decomposition, rename, or behavior-preserving change. Builds a minimal safe edit sequence before code changes."
managed-by: snap-rs
---

# snap-rs Refactor Plan

Use this when the selected phase is primarily a refactor or migration.

The goal is to preserve behavior while making the requested structural change.

## Workflow

1. Re-read the selected phase and identify the intended behavior-preserving boundary.
2. Identify current tests or commands that can detect regressions.
3. Inspect existing code shape and local patterns.
4. Define the smallest safe edit sequence.
5. Prefer mechanical, reversible steps.
6. Avoid changing public behavior unless the selected phase explicitly requires it.
7. Run checks after the refactor.
8. If behavior changes are necessary, state why they are required by the selected phase.

## Refactor Plan Shape

Before editing, form a short plan:

```md
## Refactor Plan

Goal: ...
Behavior that must not change: ...
Files likely touched: ...
Safe sequence:
1. ...
2. ...
3. ...
Verification: ...
```

## Guardrails

Do not combine unrelated cleanup with the refactor.

Do not introduce a new abstraction unless it clearly reduces real complexity in the touched area.

Do not move code across ownership boundaries unless the phase requires it.

Do not split or rename files just to make the diff look cleaner.

## Completion Criteria

The refactor is complete when:

- requested structure is achieved
- behavior is preserved or intentionally changed per phase contract
- checks pass
- diff remains focused
"#,
    },
    Skill {
        name: "snap-plan-update",
        body: r#"---
name: snap-plan-update
description: "snap-rs PLAN.md update skill. Use only when implementation or validation proves the selected PLAN.md phase has an incorrect order, missing validation gate, impossible instruction, or incomplete phase contract."
managed-by: snap-rs
---

# snap-rs PLAN.md Update

Use this only when the current `PLAN.md` is wrong or incomplete for the selected phase.

This skill is not for documenting progress or adding nice-to-have tasks.

## Valid Reasons To Update PLAN.md

Update `PLAN.md` only when one of these is true:

- the selected phase cannot be implemented safely as written
- validation proves a required step is missing
- phase order is incorrect
- validation gates are insufficient or impossible
- the phase contradicts `DESIGN.md`
- implementation exposes a prerequisite that must be part of this phase

## Workflow

1. Identify the exact phase and exact defect in the plan.
2. Confirm the issue from repo-local evidence.
3. Make the smallest correction needed.
4. Preserve the existing plan style.
5. Do not rewrite unrelated phases.
6. Do not add future features.
7. After updating, continue implementing or validating only the selected phase.

## Update Rules

Prefer:

- adding a missing validation command
- clarifying an ambiguous step
- correcting phase order locally
- marking a blocker explicitly

Avoid:

- broad plan rewrites
- new roadmap sections
- speculative future phases
- duplicating implementation details already obvious in code

## Completion Criteria

A PLAN.md update is acceptable only when it makes the selected phase implementable or verifiable.
"#,
    },
    Skill {
        name: "snap-spec-update",
        body: r#"---
name: snap-spec-update
description: "snap-rs DESIGN.md update skill. Use only when implementing or validating the selected phase exposes a real product, architecture, or behavior-contract gap in DESIGN.md."
managed-by: snap-rs
---

# snap-rs Spec Update

Use this only when `DESIGN.md` is missing or contradicting a real product or architecture decision needed by the selected phase.

This skill is not for general documentation polish.

## Valid Reasons To Update DESIGN.md

Update `DESIGN.md` only when:

- implementation exposes an undefined product behavior
- current code and phase plan reveal a design contradiction
- a phase requires a decision that belongs in the product contract
- validation cannot determine correctness without a missing contract
- the design doc is stale in a way directly affecting the selected phase

## Workflow

1. Identify the exact missing or incorrect design contract.
2. Confirm it is required by the selected phase.
3. Make the smallest possible update to `DESIGN.md`.
4. Preserve the existing document style and structure.
5. Avoid implementation chatter unless the doc already uses that style.
6. Return to the selected phase after the update.

## Guardrails

Do not use `DESIGN.md` as an implementation log.

Do not add speculative product features.

Do not rewrite unrelated design sections.

Do not make design decisions silently if the correct product behavior cannot be inferred from the phase, code, or existing docs. Mark the gap clearly instead.

## Completion Criteria

A DESIGN.md update is acceptable only when it clarifies the product or architecture contract needed to complete the selected phase.
"#,
    },
    Skill {
        name: "snap-security-review",
        body: r#"---
name: snap-security-review
description: "snap-rs focused security review skill. Use when a selected phase touches auth, secrets, shell commands, file IO, network calls, user input, dependency changes, MCP/tool configuration, agent boundaries, or other security-sensitive surfaces."
managed-by: snap-rs
---

# snap-rs Security Review

Use this when the selected phase touches security-sensitive behavior.

## Trigger Surfaces

Run this review when touched code or config involves:

- authentication or authorization
- secrets, tokens, credentials, private keys, or environment variables
- command execution or shell arguments
- file reads, writes, paths, archives, uploads, or downloads
- network calls, webhooks, callbacks, redirects, or user-controlled URLs
- user input parsing or interpolation
- database queries
- dependency, package, lockfile, or tool changes
- MCP server config, tool config, plugin config, or agent tool boundaries
- logs that may contain sensitive data
- permission, sandbox, or approval behavior

## Workflow

1. Identify security-sensitive touched surfaces.
2. Trace user-controlled or external input to dangerous sinks.
3. Check for secrets committed or newly exposed.
4. Check shell commands for injection, quoting, and untrusted arguments.
5. Check file paths for traversal, unintended overwrite, and unsafe deletion.
6. Check network calls for SSRF, open redirect, insecure transport, and credential leakage.
7. Check auth changes for missing checks, privilege escalation, and insecure defaults.
8. Check dependency changes for unpinned, unexpected, or vulnerable packages where practical.
9. Check MCP/tool config for hardcoded secrets, unsafe args, latest-style pinning, and broad permissions.
10. Fix confirmed issues that are in scope for the selected phase.
11. Report out-of-scope risks without expanding the implementation.

## Finding Standard

Do not report speculative issues as confirmed vulnerabilities.

For each confirmed finding, know:

- affected file
- vulnerable behavior
- exploit or failure path
- severity
- minimal fix
- verification performed

## Guardrails

Do not perform broad security rewrites.

Do not introduce security frameworks unless the selected phase requires them.

Do not rotate credentials or modify live services.

Do not remove test fixtures just because they look like secrets unless confirmed unsafe.

## Completion Criteria

Security review is complete when all touched security-sensitive surfaces have been checked and confirmed issues within phase scope are fixed or clearly reported.
"#,
    },
    Skill {
        name: "snap-test-gap-review",
        body: r#"---
name: snap-test-gap-review
description: "snap-rs test and verification gap review skill. Use during validation to detect weak tests, missing behavior coverage, fake confidence, skipped checks, or verification that does not prove the selected PLAN.md phase works."
managed-by: snap-rs
---

# snap-rs Test Gap Review

Use this during validation after inspecting the implementation.

The goal is to verify behavior, not implementation trivia.

## Workflow

1. Re-read the selected phase's Validation and Web validation sections.
2. Identify what behavior must be proven.
3. Inspect existing tests and checks for that behavior.
4. Identify gaps:
   - no test covers the new behavior
   - assertions are too weak
   - only happy path is covered
   - test checks implementation details instead of behavior
   - test uses fixtures that cannot fail meaningfully
   - required command was skipped
   - manual verification is claimed without evidence
5. Add or strengthen tests only where they materially improve confidence.
6. Run the relevant checks.
7. If a required check cannot run, report the blocker and residual risk.

## Verification Preference

Prefer, in order:

1. existing project test command required by `AGENTS.md`
2. selected phase validation command
3. targeted unit or integration tests
4. focused manual verification with concrete evidence
5. explicit blocker report

## Guardrails

Do not chase 100% coverage for its own sake.

Do not add fake-confidence tests.

Do not snapshot unstable output unless that is the established local pattern.

Do not broaden test infrastructure unless the selected phase requires it.

## Completion Criteria

This review is complete when the selected phase's behavior is proven by meaningful checks, or remaining verification gaps are explicitly reported.
"#,
    },
    Skill {
        name: "snap-docs-drift-review",
        body: r#"---
name: snap-docs-drift-review
description: "snap-rs documentation drift review skill. Use when implementation or validation may affect README, AGENTS.md, DESIGN.md, PLAN.md, API docs, operational docs, or other repo-local documentation."
managed-by: snap-rs
---

# snap-rs Docs Drift Review

Use this when touched behavior may make documentation stale.

The goal is to update only directly affected docs and avoid stale parallel documentation.

## Workflow

1. Identify behavior, commands, config, APIs, workflows, or contracts changed by the selected phase.
2. Search for repo-local docs that describe those areas.
3. Compare docs against actual implementation.
4. Update only docs directly affected by this phase.
5. Preserve the repo's documentation style.
6. Prefer canonical docs over adding new parallel docs.
7. Do not move product contract out of `DESIGN.md` if that is the established source.
8. Do not put implementation-plan details into user-facing docs unless the repo already does that.

## Docs To Consider

Depending on the repo, check:

- `README.md`
- `AGENTS.md`
- `DESIGN.md`
- `PLAN.md`
- docs under `docs/`
- command help text
- API or schema docs
- examples
- configuration templates

## Guardrails

Do not add docs for unchanged behavior.

Do not rewrite docs for style alone.

Do not create a new documentation system.

Do not bury product-contract changes in chat only.

## Completion Criteria

Docs drift review is complete when directly affected documentation is correct, or no docs update is needed and that conclusion is supported by inspection.
"#,
    },
    Skill {
        name: "snap-rollout-review",
        body: r#"---
name: snap-rollout-review
description: "snap-rs rollout and operational readiness review skill. Use for selected phases involving deployment, infrastructure, runtime config, migrations, observability, production behavior, or operational failure modes."
managed-by: snap-rs
---

# snap-rs Rollout Review

Use this when the selected phase affects runtime or production operations.

## Trigger Surfaces

Use this for phases involving:

- deployment
- infrastructure
- database migrations
- config or environment variables
- runtime permissions
- observability
- logging, metrics, tracing, or alerts
- rollback behavior
- background jobs or schedulers
- service dependencies
- production failure modes

## Workflow

1. Identify what operational behavior changes.
2. Check required config and defaults.
3. Check startup, shutdown, retry, timeout, and failure behavior where relevant.
4. Check observability: logs, metrics, traces, health checks, or user-visible errors.
5. Check rollback or recovery path.
6. Check migration or deploy ordering if applicable.
7. Check whether docs or runbooks need direct updates.
8. Run available preflight or validation commands.
9. Fix phase-scoped operational gaps.

## Rollout Questions

Know the answer to:

- What changes at runtime?
- What config is required?
- How would failure show up?
- How would an operator verify success?
- How would an operator roll back or recover?
- What is the smallest safe deploy order?

## Guardrails

Do not add production infrastructure unless the selected phase requires it.

Do not invent observability systems.

Do not hardcode toy assumptions into runtime paths.

Do not expand into release automation unless it is part of the selected phase.

## Completion Criteria

Rollout review is complete when runtime risk introduced by the selected phase is understood, verified where practical, and documented if needed.
"#,
    },
    Skill {
        name: "snap-dependency-review",
        body: r#"---
name: snap-dependency-review
description: "snap-rs dependency and supply-chain review skill. Use when a selected phase changes dependencies, lockfiles, package manager config, generated files, CI security config, tool versions, or plugin/MCP/tool installation."
managed-by: snap-rs
---

# snap-rs Dependency Review

Use this when the selected phase changes dependencies or tool supply chain.

## Trigger Surfaces

Use this for changes to:

- package manifests
- lockfiles
- vendored code
- generated code
- build scripts
- CI workflows that install tools
- Dockerfiles or container images
- MCP servers or plugin config
- tool versions
- dependency update policy
- scripts downloaded from the network

## Workflow

1. Identify every dependency or toolchain change.
2. Check whether the change is required by the selected phase.
3. Confirm lockfiles or equivalent generated dependency state are updated consistently.
4. Prefer pinned versions over floating versions when the repo pattern allows.
5. Watch for `latest`, unpinned Git URLs, curl-to-shell, broad install scripts, or unknown registries.
6. Check for secrets or credentials in package, tool, or CI config.
7. Run dependency-related checks available in the repo.
8. Report out-of-scope supply-chain risks without expanding the phase.

## Guardrails

Do not upgrade unrelated dependencies.

Do not normalize the whole lockfile unless the selected phase requires it.

Do not add scanners or services unless already part of the repo or phase.

Do not trust generated code blindly; inspect whether it is intended to be committed.

## Completion Criteria

Dependency review is complete when dependency/tool changes are necessary, consistent, pinned where appropriate, and verified by available checks.
"#,
    },
    Skill {
        name: "snap-final-review",
        body: r#"---
name: snap-final-review
description: "snap-rs final phase closeout skill. Use at the end of validation to review the final diff, confirm the selected phase contract is satisfied, summarize verification, and flag out-of-scope follow-ups without expanding work."
managed-by: snap-rs
---

# snap-rs Final Review

Use this at the end of the validation pass.

The goal is to close the selected phase cleanly.

## Workflow

1. Re-read the selected phase contract.
2. Review the final diff.
3. Confirm each required behavior is implemented.
4. Confirm tests or validation checks were run.
5. Confirm fixes stayed within selected-phase scope.
6. Confirm no later-phase work was added.
7. Confirm docs were updated only if directly affected.
8. Confirm security, dependency, rollout, and test-gap reviews were used when triggered.
9. Identify out-of-scope issues separately without fixing them.
10. Produce a concise final summary.

## Final Summary Shape

Use this structure:

```md
## Phase Closeout

Implemented:
- ...

Verified:
- ...

Changed docs:
- ...

Not done / blocked:
- ...

Out-of-scope follow-ups:
- ...
```

Omit sections that do not apply.

## Guardrails

Do not make new edits during final review unless they are required to complete the selected phase.

Do not hide failed or skipped checks.

Do not claim validation that was not performed.

Do not commit or push unless explicitly requested.

## Completion Criteria

Final review is complete when the selected phase can be honestly reported as complete, or the remaining blocker is explicit and actionable.
"#,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(body.contains(MANAGED_MARKER), "{}", skill.name);
            assert!(
                body.contains(&format!("name: {}", skill.name)),
                "{}",
                skill.name
            );
        }

        let gitignore = fs::read_to_string(temp.path().join(".gitignore")).expect("gitignore");
        assert_eq!(
            gitignore
                .lines()
                .filter(|line| line.trim() == GITIGNORE_ENTRY)
                .count(),
            1
        );
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
}
