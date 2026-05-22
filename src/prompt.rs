use std::path::Path;

pub fn implementation_prompt(
    plan_path: &Path,
    agents_path: &Path,
    design_path: &Path,
    phase: u32,
    title: &str,
) -> String {
    format!(
        "{}\n\n{}\n",
        context_docs_block(plan_path, agents_path, design_path),
        phase_task_block(plan_path, phase, title, PhaseTask::Implement)
    )
}

pub fn validation_prompt(
    plan_path: &Path,
    agents_path: &Path,
    design_path: &Path,
    phase: u32,
    title: &str,
) -> String {
    format!(
        "{}\n\n{}\n",
        context_docs_block(plan_path, agents_path, design_path),
        phase_task_block(plan_path, phase, title, PhaseTask::Validate)
    )
}

pub fn review_prompt(
    plan_path: &Path,
    agents_path: &Path,
    design_path: &Path,
    phase: u32,
    title: &str,
) -> String {
    format!(
        "{}\n\n{}\n",
        context_docs_block(plan_path, agents_path, design_path),
        phase_task_block(plan_path, phase, title, PhaseTask::Review)
    )
}

fn context_docs_block(plan_path: &Path, agents_path: &Path, design_path: &Path) -> String {
    format!(
        "\
Read these context files before coding:
- {agents}
- {design}
- {plan}
Treat {design} as the product contract and {plan} as the implementation order.
Treat {agents} as the authoritative agent instructions for this run.
Use only repo-local files and official docs relevant to the selected phase.
When validating version-sensitive Rust, Cargo, Git, dependency, or test-runner behavior, check the installed version and repo-local config first; use current official docs when local evidence is not enough.",
        agents = agents_path.display(),
        design = design_path.display(),
        plan = plan_path.display(),
    )
}

#[derive(Debug, Clone, Copy)]
enum PhaseTask {
    Implement,
    Validate,
    Review,
}

fn phase_task_block(plan_path: &Path, phase: u32, title: &str, task: PhaseTask) -> String {
    match task {
        PhaseTask::Implement => format!(
            "\
Open {plan} and locate exactly:

## Phase {phase} - {title}

Use $snap-context-map before editing.
Use $snap-phase-implement for the implementation pass.
Use $snap-technical-spike if the phase depends on unknown or version-sensitive behavior.
Use $snap-refactor-plan if the phase is a refactor, migration, cleanup, decomposition, rename, or behavior-preserving change.
Use $snap-cli-control if the phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos.
Use $snap-ui-control if the phase changes browser, Electron, or local UI behavior.
Use $snap-security-review if the phase touches auth, secrets, command execution, file IO, network calls, user input, dependencies, MCP/tool config, or agent/tool boundaries.
Use $snap-plan-update only if PLAN.md needs a correction to make this selected phase implementable or verifiable.
Use $snap-spec-update only if DESIGN.md has a real product or architecture contract gap exposed by this selected phase.

Implement Phase {phase} completely in the current target repo. Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
        ),
        PhaseTask::Validate => format!(
            "\
Open {plan} and locate exactly:

## Phase {phase} - {title}

Use $snap-phase-validate for the validation pass.
Use $snap-test-gap-review to check whether verification proves the selected phase works.
Use $snap-security-review if the phase touches auth, secrets, command execution, file IO, network calls, user input, dependencies, MCP/tool config, or agent/tool boundaries.
Use $snap-docs-drift-review if changed behavior may affect README, AGENTS.md, DESIGN.md, PLAN.md, API docs, operational docs, examples, or command help.
Use $snap-rollout-review if the phase affects deployment, infrastructure, runtime config, migrations, observability, rollback, or production failure modes.
Use $snap-dependency-review if the phase changes dependencies, lockfiles, package manager config, generated files, CI security config, tool versions, or plugin/MCP/tool installation.

Validate that Phase {phase} was implemented fully and correctly in the current target repo.
Fix only correctness, test, docs, security, dependency, or rollout gaps needed to complete this selected phase.
Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
        ),
        PhaseTask::Review => format!(
            "\
Open {plan} and locate exactly:

## Phase {phase} - {title}

Use $snap-phase-review for the local phase review pass.
Use $snap-cli-control if the phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos and validation did not already prove the user-visible behavior.
Use $snap-ui-control if the phase changes browser, Electron, or local UI behavior and validation did not already prove the user-visible behavior.
Use $snap-final-review before finishing.

Review Phase {phase} in the current target repo after implementation and validation.
Fix only small, high-confidence, phase-scoped review findings.
If a finding requires broad redesign, unrelated refactor, new product behavior, PR/CI workflow, or later-phase work, report it as out of scope or blocked instead of fixing it.
Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implementation_prompt_preserves_phase_contract() {
        let prompt = implementation_prompt(
            Path::new("PLAN.md"),
            Path::new("AGENTS.md"),
            Path::new("DESIGN.md"),
            4,
            "Path And Environment Resolution",
        );

        assert!(prompt.contains("Treat DESIGN.md as the product contract"));
        assert!(prompt.contains("## Phase 4 - Path And Environment Resolution"));
        assert!(prompt.contains("$snap-context-map"));
        assert!(prompt.contains("$snap-phase-implement"));
        assert!(prompt.contains("$snap-technical-spike"));
        assert!(prompt.contains("$snap-refactor-plan"));
        assert!(prompt.contains("$snap-cli-control"));
        assert!(prompt.contains("$snap-ui-control"));
        assert!(prompt.contains("$snap-security-review"));
        assert!(prompt.contains("$snap-plan-update"));
        assert!(prompt.contains("$snap-spec-update"));
        assert!(prompt.contains("current target repo"));
        assert!(prompt.contains("Do not commit or push"));
    }

    #[test]
    fn validation_prompt_requires_independent_evidence_based_check() {
        let prompt = validation_prompt(
            Path::new("PLAN.md"),
            Path::new("AGENTS.md"),
            Path::new("DESIGN.md"),
            2,
            "Verification Loop",
        );

        assert!(prompt.contains("## Phase 2 - Verification Loop"));
        assert!(prompt.contains("$snap-phase-validate"));
        assert!(prompt.contains("$snap-test-gap-review"));
        assert!(!prompt.contains("$snap-final-review"));
        assert!(prompt.contains("$snap-security-review"));
        assert!(prompt.contains("$snap-docs-drift-review"));
        assert!(prompt.contains("$snap-rollout-review"));
        assert!(prompt.contains("$snap-dependency-review"));
        assert!(prompt.contains("current target repo"));
    }

    #[test]
    fn review_prompt_runs_quality_review_and_final_closeout() {
        let prompt = review_prompt(
            Path::new("PLAN.md"),
            Path::new("AGENTS.md"),
            Path::new("DESIGN.md"),
            3,
            "Output Polish",
        );

        assert!(prompt.contains("## Phase 3 - Output Polish"));
        assert!(prompt.contains("$snap-phase-review"));
        assert!(prompt.contains("$snap-final-review"));
        assert!(prompt.contains("$snap-cli-control"));
        assert!(prompt.contains("$snap-ui-control"));
        assert!(prompt.contains("Fix only small, high-confidence, phase-scoped"));
        assert!(prompt.contains("broad redesign"));
    }
}
