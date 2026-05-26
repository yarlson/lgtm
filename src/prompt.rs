use crate::phase_index::Phase;
use crate::skills;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhasePass {
    Implement,
    Validate,
    Review,
}

impl PhasePass {
    pub const ALL: [Self; 3] = [Self::Implement, Self::Validate, Self::Review];

    pub fn action(self) -> &'static str {
        match self {
            Self::Implement => "implement",
            Self::Validate => "validate",
            Self::Review => "review",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Implement => "implementation",
            Self::Validate => "validation",
            Self::Review => "review",
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
        context_docs_block(plan_path, agents_path),
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
- Use sequential phase numbers.
- Every phase must include actionable Goal, Steps, and Validation content.
- Every non-cleanup implementation phase must also include actionable Acceptance Criteria and Artifacts content inside the phase.
- Define plan-level Runner expectations, Anti-patterns, and Final State so downstream phases understand execution boundaries.
- Do not require Runner, Anti-patterns, or Final State labels in every phase.
- Require explicit cleanup phases when a plan crosses meaningful risk boundaries such as migrations, broad refactors, dependency changes, CLI/TUI behavior changes, major test rewrites, or generated artifact churn.
- Cleanup phases are optional for small, low-risk plans; never insert them on a mechanical every-N-phases schedule.
- Cleanup phase guidance should cover reviewability, test stabilization, docs drift, leftover compatibility paths, and PLAN_STATUS.md reconciliation.
- Cleanup phases use normal sequential ## Phase N - Name headings and are executed like any other phase.
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
Use the required final plan contract: # Plan, sequential ## Phase N - Name headings, and per-phase Goal:, Steps:, and Validation: blocks.
For every non-cleanup implementation phase, include actionable Acceptance Criteria, Artifacts, and Validation content inside the phase.
Define plan-level Runner, Anti-patterns, and Final State expectations without requiring those labels in every phase.
Include explicit cleanup phases when the plan crosses meaningful risk boundaries such as migrations, broad refactors, dependency changes, CLI/TUI behavior changes, major test rewrites, or generated artifact churn.
Do not add cleanup phases on a mechanical every-N-phases schedule; keep them optional for small, low-risk plans.
Make cleanup phases cover reviewability, test stabilization, docs drift, leftover compatibility paths, and PLAN_STATUS.md reconciliation when those risks apply.
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

fn context_docs_block(plan_path: &std::path::Path, agents_path: &std::path::Path) -> String {
    format!(
        "\
Read these context files before coding:
- {agents}
- {plan}
Treat {plan} as the implementation order and the source of any linked project-contract context.
Treat {agents} as the authoritative agent instructions for this run.
Use only repo-local files and official docs relevant to the selected phase.
When validating version-sensitive Rust, Cargo, Git, dependency, or test-runner behavior, check the installed version and repo-local config first; use current official docs when local evidence is not enough.

Runtime plan/status rules:
- Treat {plan} as immutable after /finish. Do not edit it for ordinary progress, status, discoveries, or later-phase notes.
- Create root-level PLAN_STATUS.md lazily during the first run pass if it is missing.
- Append or update concise selected-phase progress, verification, blockers, and final phase status in PLAN_STATUS.md during each implement, validate, and review pass.
- Use $lgtm-plan-update only for an exceptional selected-phase contract defect that makes {plan} impossible or unsafe to execute as written.",
        agents = agents_path.display(),
        plan = plan_path.display(),
    )
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
Use $lgtm-spec-update only if the selected phase exposes a real product or architecture contract gap that belongs in a doc linked from PLAN.md.

Maintain PLAN_STATUS.md for this implementation pass: create it if needed, then record concise progress, verification, blockers, and the current implementation status for Phase {number}.

Implement Phase {number} completely in the current target repo. Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
            number = phase.id,
            heading = phase.heading,
        ),
        PhasePass::Validate => format!(
            "\
Open {plan} and locate this exact selected phase heading:
{heading}

Use $lgtm-phase-validate for the validation pass.
Use $lgtm-test-gap-review to check whether verification proves the selected phase works.
Use $lgtm-security-review if the phase touches auth, secrets, command execution, file IO, network calls, user input, dependencies, MCP/tool config, or agent/tool boundaries.
Use $lgtm-docs-drift-review if changed behavior may affect README, AGENTS.md, linked project docs, API docs, operational docs, examples, or command help.
Use $lgtm-rollout-review if the phase affects deployment, infrastructure, runtime config, migrations, observability, rollback, or production failure modes.
Use $lgtm-dependency-review if the phase changes dependencies, lockfiles, package manager config, generated files, CI security config, tool versions, or plugin/MCP/tool installation.

Maintain PLAN_STATUS.md for this validation pass: create it if needed, then record concise progress, verification, blockers, and the current validation status for Phase {number}.

Validate that Phase {number} was implemented fully and correctly in the current target repo.
Fix only correctness, test, docs, security, dependency, or rollout gaps needed to complete this selected phase.
Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
            number = phase.id,
            heading = phase.heading,
        ),
        PhasePass::Review => format!(
            "\
Open {plan} and locate this exact selected phase heading:
{heading}

Use $lgtm-phase-review for the local phase review pass.
Use $lgtm-cli-control if the phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos and validation did not already prove the user-visible behavior.
Use $lgtm-ui-control if the phase changes browser, Electron, or local UI behavior and validation did not already prove the user-visible behavior.
Use $lgtm-final-review before finishing.

Maintain PLAN_STATUS.md for this review pass: create it if needed, then record concise progress, verification, blockers, and the final review status for Phase {number}.

Review Phase {number} in the current target repo after implementation and validation.
Fix only small, high-confidence, phase-scoped review findings.
If a finding requires broad redesign, unrelated refactor, new product behavior, PR/CI workflow, or later-phase work, report it as out of scope or blocked instead of fixing it.
Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
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
    fn phase_prompts_include_plan_status_runtime_guidance() {
        let phase = Phase {
            id: 2,
            title: "Runtime Guidance".to_string(),
            heading: "## Phase 2 - Runtime Guidance".to_string(),
        };

        for pass in PhasePass::ALL {
            let prompt = phase_prompt(
                std::path::Path::new("PLAN.md"),
                std::path::Path::new("AGENTS.md"),
                &phase,
                pass,
            );

            assert!(prompt.contains("Treat PLAN.md as immutable after /finish"));
            assert!(prompt.contains("Do not edit it for ordinary progress, status, discoveries"));
            assert!(prompt.contains("Create root-level PLAN_STATUS.md lazily"));
            assert!(prompt.contains("during each implement, validate, and review pass"));
            assert!(prompt.contains(
                "Use $lgtm-plan-update only for an exceptional selected-phase contract defect"
            ));
            assert!(prompt.contains("Maintain PLAN_STATUS.md"));
            assert!(prompt.contains("verification, blockers"));
        }
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
        assert!(prompt.contains("Every non-cleanup implementation phase"));
        assert!(prompt.contains("Acceptance Criteria"));
        assert!(prompt.contains("Artifacts"));
        assert!(prompt.contains("Runner expectations"));
        assert!(prompt.contains("Anti-patterns"));
        assert!(prompt.contains("Final State"));
        assert!(prompt.contains("Do not require Runner, Anti-patterns, or Final State labels"));
        assert!(prompt.contains("Require explicit cleanup phases"));
        assert!(prompt.contains("meaningful risk boundaries"));
        assert!(prompt.contains("migrations, broad refactors, dependency changes"));
        assert!(prompt.contains("CLI/TUI behavior changes"));
        assert!(prompt.contains("generated artifact churn"));
        assert!(prompt.contains("optional for small, low-risk plans"));
        assert!(prompt.contains("never insert them on a mechanical every-N-phases schedule"));
        assert!(prompt.contains("reviewability, test stabilization, docs drift"));
        assert!(prompt.contains("leftover compatibility paths"));
        assert!(prompt.contains("PLAN_STATUS.md reconciliation"));
        assert!(prompt.contains("executed like any other phase"));
        assert!(prompt.contains("User brief:\nsplit the migration"));
    }

    #[test]
    fn resume_plan_prompt_only_special_cases_exact_finish() {
        assert!(plan_resume_prompt("/finish").contains("Write the final PLAN.md now"));
        assert!(plan_resume_prompt("/finish").contains("Acceptance Criteria"));
        assert!(plan_resume_prompt("/finish").contains("Final State expectations"));
        assert!(plan_resume_prompt("/finish").contains("meaningful risk boundaries"));
        assert!(plan_resume_prompt("/finish").contains("mechanical every-N-phases schedule"));
        assert!(plan_resume_prompt("/finish").contains("PLAN_STATUS.md reconciliation"));
        assert_eq!(plan_resume_prompt(" /finish "), " /finish ");
        assert_eq!(plan_resume_prompt("answer"), "answer");
    }
}
