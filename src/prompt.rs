use crate::phase_index::Phase;
use crate::skills;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhasePass {
    Implement,
    Validate,
    Review,
    Commit,
}

impl PhasePass {
    pub const ALL: [Self; 4] = [Self::Implement, Self::Validate, Self::Review, Self::Commit];

    pub fn action(self) -> &'static str {
        match self {
            Self::Implement => "implement",
            Self::Validate => "validate",
            Self::Review => "review",
            Self::Commit => "commit",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Implement => "implementation",
            Self::Validate => "validation",
            Self::Review => "review",
            Self::Commit => "commit",
        }
    }

    pub fn reasoning_effort(self) -> &'static str {
        match self {
            Self::Implement | Self::Review => "high",
            Self::Validate => "medium",
            Self::Commit => "low",
        }
    }
}

pub fn phase_prompt(
    plan_path: &std::path::Path,
    agents_path: &std::path::Path,
    phase: &Phase,
    pass: PhasePass,
) -> String {
    format!(
        "{}\n\n{}\n",
        phase_context_block(plan_path, agents_path, pass),
        phase_task_block(plan_path, phase, pass)
    )
}

pub fn plan_initial_prompt(
    plan_path: &std::path::Path,
    agents_path: &std::path::Path,
    brief: Option<&str>,
) -> String {
    format!(
        "\
Use ${plan_create} for this planning session.

Target PLAN.md path: {plan}
Target AGENTS.md path: {agents}

Read {agents} only if it exists in the target repository. Do not require {agents} to exist.

Planning rules:
- Ask exactly one sharp question per turn.
- Ask questions by writing a normal assistant message only; do not call request_user_input or any interactive input tool.
- Prefer forced choices over open-ended questions.
- If the user answer is vague, reject it and ask one narrower follow-up.
- Keep planning state in the Codex session, not in draft files.
- PLAN.md is a final-only sentinel: do not create or modify it as a draft.
- Write PLAN.md only when ready to finish.
- Preserve an existing {agents}; if {agents} is missing, create it when writing the final PLAN.md.
- Before creating {agents}, detect the project stack from repo files and web-search current-year best practices for that stack.
- Keep generated {agents} practical, repo-local, and focused on engineering workflow, coding rules, validation, and safety constraints.

Final PLAN.md contract:
- # Plan
- ## Phase N - Name
- Goal:
- Steps:
- Validation:
{brief_block}",
        plan_create = skills::PLAN_CREATE,
        plan = plan_path.display(),
        agents = agents_path.display(),
        brief_block = plan_brief_block(brief),
    )
}

pub fn plan_resume_prompt(answer: &str) -> String {
    if answer == "/finish" {
        "\
The user requested /finish.
Write the final PLAN.md now at the requested path. If AGENTS.md was missing at the start of planning, create it now too.
Produce the best plan possible from the current session context, mark unresolved risks explicitly, and do not invent certainty."
            .to_string()
    } else {
        answer.to_string()
    }
}

fn plan_brief_block(brief: Option<&str>) -> String {
    match brief {
        Some(brief) if !brief.trim().is_empty() => {
            format!("\n\nUser brief:\n{brief}", brief = brief.trim())
        }
        _ => String::new(),
    }
}

fn phase_context_block(
    plan_path: &std::path::Path,
    agents_path: &std::path::Path,
    pass: PhasePass,
) -> String {
    match pass {
        PhasePass::Implement => format!(
            "\
This is the first turn for the selected phase.
Read these context files before coding:
- {agents}
- {plan}
Treat {plan} as the implementation order and the source of any linked project-contract context.
Treat {agents} as the authoritative agent instructions for this run.
Use only repo-local files and official docs relevant to the selected phase.
When validating version-sensitive Rust, Cargo, Git, dependency, or test-runner behavior, check the installed version and repo-local config first; use current official docs when local evidence is not enough.
Keep the final response concise: changes, verification, blockers only.",
            agents = agents_path.display(),
            plan = plan_path.display(),
        ),
        PhasePass::Validate | PhasePass::Review | PhasePass::Commit => format!(
            "\
Continue the same Codex session for this selected phase.
Use the {agents}, {plan}, selected phase, and implementation context already established in this session.
Re-open repo files only when needed to verify current state, inspect diffs, or resolve uncertainty.
Do not redo broad codebase discovery unless earlier session context is missing or stale.
Keep the final response concise: findings, fixes, verification, blockers only.",
            agents = agents_path.display(),
            plan = plan_path.display(),
        ),
    }
}

fn phase_task_block(plan_path: &std::path::Path, phase: &Phase, pass: PhasePass) -> String {
    match pass {
        PhasePass::Implement => format!(
            "\
Open {plan} and locate this exact selected phase heading:
{heading}

Use $lgtm-context-map before editing.
Use $lgtm-phase-implement for the implementation pass.
Use $lgtm-technical-spike if the phase depends on unknown or version-sensitive behavior.
Use $lgtm-refactor-plan if the phase is a refactor, migration, cleanup, decomposition, rename, or behavior-preserving change.
Use $lgtm-cli-control if the phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos.
Use $lgtm-ui-control if the phase changes browser, Electron, or local UI behavior.
Use $lgtm-security-review if the phase touches auth, secrets, command execution, file IO, network calls, user input, dependencies, MCP/tool config, or agent/tool boundaries.
Use $lgtm-plan-update only if PLAN.md needs a correction to make this selected phase implementable or verifiable.
Use $lgtm-spec-update only if the selected phase exposes a real product or architecture contract gap that belongs in PLAN.md or a doc linked from PLAN.md.

Implement Phase {number} completely in the current target repo. Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
            number = phase.id,
            heading = phase.heading,
        ),
        PhasePass::Validate => format!(
            "\
Continue with this exact selected phase heading:
{heading}

Use $lgtm-phase-validate for the validation pass.
Use $lgtm-test-gap-review to check whether verification proves the selected phase works.
Use $lgtm-security-review if the phase touches auth, secrets, command execution, file IO, network calls, user input, dependencies, MCP/tool config, or agent/tool boundaries.
Use $lgtm-docs-drift-review if changed behavior may affect README, AGENTS.md, PLAN.md, linked project docs, API docs, operational docs, examples, or command help.
Use $lgtm-rollout-review if the phase affects deployment, infrastructure, runtime config, migrations, observability, rollback, or production failure modes.
Use $lgtm-dependency-review if the phase changes dependencies, lockfiles, package manager config, generated files, CI security config, tool versions, or plugin/MCP/tool installation.

Validate that Phase {number} was implemented fully and correctly in the current target repo.
Fix only correctness, test, docs, security, dependency, or rollout gaps needed to complete this selected phase.
Do not commit or push unless the user explicitly requested it for this run.",
            number = phase.id,
            heading = phase.heading,
        ),
        PhasePass::Review => format!(
            "\
Continue with this exact selected phase heading:
{heading}

Use $lgtm-phase-review for the local phase review pass.
Use $lgtm-refactor-plan if a review finding has a clear behavior-preserving restructuring path that is larger than a trivial edit.
Use $lgtm-cli-control if the phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos and validation did not already prove the user-visible behavior.
Use $lgtm-ui-control if the phase changes browser, Electron, or local UI behavior and validation did not already prove the user-visible behavior.
Use $lgtm-final-review before finishing.

Review Phase {number} in the current target repo after implementation and validation.
Run a strict structural maintainability review, then fix every safe phase-scoped finding before finishing.
If a finding requires broad redesign, unrelated refactor, new product behavior, PR/CI workflow, or later-phase work, report it as out of scope or blocked instead of fixing it.
Do not commit or push unless the user explicitly requested it for this run.",
            number = phase.id,
            heading = phase.heading,
        ),
        PhasePass::Commit => format!(
            "\
Continue with this exact selected phase heading:
{heading}

Use $lgtm-phase-commit for the after-phase commit pass.

Commit Phase {number} after implementation, validation, and review are complete.
Inspect git status plus staged and unstaged diffs.
Stage all changes.
Create a real git commit with a rich message: concise subject, body summary of the phase scope, key changes, verification performed, and any blockers or skipped checks.
Do not create an empty commit. If there are no changes to commit, report that explicitly.
Do not push, create branches, open PRs, manage CI, or release tags.",
            number = phase.id,
            heading = phase.heading,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implementation_prompt_includes_phase_and_skills() {
        let prompt = phase_prompt(
            std::path::Path::new("PLAN.md"),
            std::path::Path::new("AGENTS.md"),
            &Phase {
                id: 4,
                title: "Path And Environment Resolution".to_string(),
                heading: "## Phase 4 - Path And Environment Resolution".to_string(),
            },
            PhasePass::Implement,
        );

        assert!(prompt.contains("## Phase 4 - Path And Environment Resolution"));
        assert!(prompt.contains("$lgtm-context-map"));
        assert!(prompt.contains("$lgtm-phase-implement"));
        assert!(prompt.contains("$lgtm-refactor-plan"));
        assert!(prompt.contains("Do not commit or push"));
    }

    #[test]
    fn phase_passes_include_commit_after_review() {
        assert_eq!(
            PhasePass::ALL,
            [
                PhasePass::Implement,
                PhasePass::Validate,
                PhasePass::Review,
                PhasePass::Commit
            ]
        );
        assert_eq!(PhasePass::Commit.action(), "commit");
        assert_eq!(PhasePass::Commit.label(), "commit");
    }

    #[test]
    fn phase_passes_use_targeted_reasoning_effort() {
        assert_eq!(PhasePass::Implement.reasoning_effort(), "high");
        assert_eq!(PhasePass::Validate.reasoning_effort(), "medium");
        assert_eq!(PhasePass::Review.reasoning_effort(), "high");
        assert_eq!(PhasePass::Commit.reasoning_effort(), "low");
    }

    #[test]
    fn commit_prompt_includes_phase_and_guardrails() {
        let prompt = phase_prompt(
            std::path::Path::new("PLAN.md"),
            std::path::Path::new("AGENTS.md"),
            &Phase {
                id: 4,
                title: "Path And Environment Resolution".to_string(),
                heading: "## Phase 4 - Path And Environment Resolution".to_string(),
            },
            PhasePass::Commit,
        );

        assert!(prompt.contains("## Phase 4 - Path And Environment Resolution"));
        assert!(prompt.contains("$lgtm-phase-commit"));
        assert!(prompt.contains("Stage all changes"));
        assert!(prompt.contains("Create a real git commit with a rich message"));
        assert!(prompt.contains("Do not create an empty commit"));
        assert!(prompt.contains("Do not push, create branches, open PRs, manage CI"));
        assert!(!prompt.contains("safely separated"));
    }

    #[test]
    fn review_prompt_requires_strict_fixing_review() {
        let prompt = phase_prompt(
            std::path::Path::new("PLAN.md"),
            std::path::Path::new("AGENTS.md"),
            &Phase {
                id: 4,
                title: "Path And Environment Resolution".to_string(),
                heading: "## Phase 4 - Path And Environment Resolution".to_string(),
            },
            PhasePass::Review,
        );

        assert!(prompt.contains("$lgtm-phase-review"));
        assert!(prompt.contains("$lgtm-refactor-plan"));
        assert!(prompt.contains("strict structural maintainability review"));
        assert!(prompt.contains("fix every safe phase-scoped finding"));
        assert!(prompt.contains("out of scope or blocked"));
    }

    #[test]
    fn initial_plan_prompt_sets_final_artifact_contract() {
        let prompt = plan_initial_prompt(
            std::path::Path::new("docs/PLAN.md"),
            std::path::Path::new("AGENTS.md"),
            Some("  split the migration  "),
        );

        assert!(prompt.contains("$lgtm-plan-create"));
        assert!(prompt.contains("Target PLAN.md path: docs/PLAN.md"));
        assert!(prompt.contains("Target AGENTS.md path: AGENTS.md"));
        assert!(prompt.contains("Do not require AGENTS.md to exist"));
        assert!(prompt.contains("do not call request_user_input"));
        assert!(prompt.contains("PLAN.md is a final-only sentinel"));
        assert!(prompt.contains("## Phase N - Name"));
        assert!(prompt.contains("User brief:\nsplit the migration"));
    }

    #[test]
    fn resume_plan_prompt_only_special_cases_exact_finish() {
        assert!(plan_resume_prompt("/finish").contains("Write the final PLAN.md now"));
        assert_eq!(plan_resume_prompt(" /finish "), " /finish ");
        assert_eq!(plan_resume_prompt("answer"), "answer");
    }
}
