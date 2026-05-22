use std::path::Path;

use crate::plan::Phase;
use crate::skills;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

pub fn phase_prompt(
    plan_path: &Path,
    agents_path: &Path,
    design_path: &Path,
    phase: &Phase,
    pass: PhasePass,
) -> String {
    format!(
        "{}\n\n{}\n",
        context_docs_block(plan_path, agents_path, design_path),
        phase_task_block(plan_path, phase, pass)
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

fn phase_task_block(plan_path: &Path, phase: &Phase, pass: PhasePass) -> String {
    match pass {
        PhasePass::Implement => format!(
            "\
Open {plan} and locate exactly:

{heading}

Use ${context_map} before editing.
Use ${phase_implement} for the implementation pass.
Use ${technical_spike} if the phase depends on unknown or version-sensitive behavior.
Use ${refactor_plan} if the phase is a refactor, migration, cleanup, decomposition, rename, or behavior-preserving change.
Use ${cli_control} if the phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos.
Use ${ui_control} if the phase changes browser, Electron, or local UI behavior.
Use ${security_review} if the phase touches auth, secrets, command execution, file IO, network calls, user input, dependencies, MCP/tool config, or agent/tool boundaries.
Use ${plan_update} only if PLAN.md needs a correction to make this selected phase implementable or verifiable.
Use ${spec_update} only if DESIGN.md has a real product or architecture contract gap exposed by this selected phase.

Implement Phase {number} completely in the current target repo. Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
            heading = phase.heading.as_str(),
            number = phase.number,
            context_map = skills::CONTEXT_MAP,
            phase_implement = skills::PHASE_IMPLEMENT,
            technical_spike = skills::TECHNICAL_SPIKE,
            refactor_plan = skills::REFACTOR_PLAN,
            cli_control = skills::CLI_CONTROL,
            ui_control = skills::UI_CONTROL,
            security_review = skills::SECURITY_REVIEW,
            plan_update = skills::PLAN_UPDATE,
            spec_update = skills::SPEC_UPDATE,
        ),
        PhasePass::Validate => format!(
            "\
Open {plan} and locate exactly:

{heading}

Use ${phase_validate} for the validation pass.
Use ${test_gap_review} to check whether verification proves the selected phase works.
Use ${security_review} if the phase touches auth, secrets, command execution, file IO, network calls, user input, dependencies, MCP/tool config, or agent/tool boundaries.
Use ${docs_drift_review} if changed behavior may affect README, AGENTS.md, DESIGN.md, PLAN.md, API docs, operational docs, examples, or command help.
Use ${rollout_review} if the phase affects deployment, infrastructure, runtime config, migrations, observability, rollback, or production failure modes.
Use ${dependency_review} if the phase changes dependencies, lockfiles, package manager config, generated files, CI security config, tool versions, or plugin/MCP/tool installation.

Validate that Phase {number} was implemented fully and correctly in the current target repo.
Fix only correctness, test, docs, security, dependency, or rollout gaps needed to complete this selected phase.
Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
            heading = phase.heading.as_str(),
            number = phase.number,
            phase_validate = skills::PHASE_VALIDATE,
            test_gap_review = skills::TEST_GAP_REVIEW,
            security_review = skills::SECURITY_REVIEW,
            docs_drift_review = skills::DOCS_DRIFT_REVIEW,
            rollout_review = skills::ROLLOUT_REVIEW,
            dependency_review = skills::DEPENDENCY_REVIEW,
        ),
        PhasePass::Review => format!(
            "\
Open {plan} and locate exactly:

{heading}

Use ${phase_review} for the local phase review pass.
Use ${cli_control} if the phase changes CLI/TUI behavior, terminal output, prompts, interrupts, hangs, resize behavior, or terminal demos and validation did not already prove the user-visible behavior.
Use ${ui_control} if the phase changes browser, Electron, or local UI behavior and validation did not already prove the user-visible behavior.
Use ${final_review} before finishing.

Review Phase {number} in the current target repo after implementation and validation.
Fix only small, high-confidence, phase-scoped review findings.
If a finding requires broad redesign, unrelated refactor, new product behavior, PR/CI workflow, or later-phase work, report it as out of scope or blocked instead of fixing it.
Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
            heading = phase.heading.as_str(),
            number = phase.number,
            phase_review = skills::PHASE_REVIEW,
            cli_control = skills::CLI_CONTROL,
            ui_control = skills::UI_CONTROL,
            final_review = skills::FINAL_REVIEW,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implementation_prompt_preserves_phase_contract() {
        let prompt = phase_prompt(
            Path::new("PLAN.md"),
            Path::new("AGENTS.md"),
            Path::new("DESIGN.md"),
            &phase(4, "Path And Environment Resolution", '-'),
            PhasePass::Implement,
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
        let prompt = phase_prompt(
            Path::new("PLAN.md"),
            Path::new("AGENTS.md"),
            Path::new("DESIGN.md"),
            &phase(2, "Verification Loop", '-'),
            PhasePass::Validate,
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
        let prompt = phase_prompt(
            Path::new("PLAN.md"),
            Path::new("AGENTS.md"),
            Path::new("DESIGN.md"),
            &phase(3, "Output Polish", '-'),
            PhasePass::Review,
        );

        assert!(prompt.contains("## Phase 3 - Output Polish"));
        assert!(prompt.contains("$snap-phase-review"));
        assert!(prompt.contains("$snap-final-review"));
        assert!(prompt.contains("$snap-cli-control"));
        assert!(prompt.contains("$snap-ui-control"));
        assert!(prompt.contains("Fix only small, high-confidence, phase-scoped"));
        assert!(prompt.contains("broad redesign"));
    }

    #[test]
    fn prompts_preserve_original_phase_heading_separator() {
        let prompt = phase_prompt(
            Path::new("PLAN.md"),
            Path::new("AGENTS.md"),
            Path::new("DESIGN.md"),
            &phase(12, "Polish", ':'),
            PhasePass::Implement,
        );

        assert!(prompt.contains("## Phase 12: Polish"));
        assert!(!prompt.contains("## Phase 12 - Polish"));
    }

    fn phase(number: u32, title: &str, separator: char) -> Phase {
        let heading = match separator {
            ':' => format!("## Phase {number}: {title}"),
            '-' => format!("## Phase {number} - {title}"),
            _ => unreachable!("unsupported test phase separator"),
        };

        Phase {
            number,
            title: title.to_string(),
            heading,
        }
    }
}
