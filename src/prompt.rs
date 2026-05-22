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
        phase_task_block(
            plan_path,
            design_path,
            agents_path,
            phase,
            title,
            PhaseTask::Implement
        )
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
        phase_task_block(
            plan_path,
            design_path,
            agents_path,
            phase,
            title,
            PhaseTask::Validate
        )
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
When validating version-sensitive Rust, Cargo, Git, dependency, or test-runner behavior, search current official docs first and include 2026 in web searches.",
        agents = agents_path.display(),
        design = design_path.display(),
        plan = plan_path.display(),
    )
}

#[derive(Debug, Clone, Copy)]
enum PhaseTask {
    Implement,
    Validate,
}

fn phase_task_block(
    plan_path: &Path,
    design_path: &Path,
    agents_path: &Path,
    phase: u32,
    title: &str,
    task: PhaseTask,
) -> String {
    match task {
        PhaseTask::Implement => format!(
            "\
Open {plan} and locate exactly:

## Phase {phase} - {title}

Implement Phase {phase} completely in the current lnk repo.
Use the phase's Goal, Steps, Validation, and Web validation sections as the task contract.
Before coding, inspect the current repo state and the files relevant to this phase.
Do not skip ahead into later phases unless the selected phase explicitly requires a small prerequisite.
Keep modules small and concern-based; do not recreate the deleted Go shape or large service files.
Do not add commands, flags, configuration, workflows, CI, release automation, or features outside {design} and the selected phase.
Update {design} only if implementation exposes a real product-design gap.
Update {plan} only if the selected phase needs a corrected implementation order or validation gate.
Run the checks required by {agents} and the selected phase. Fix failures.
If a required tool such as cargo-nextest is missing, install it when practical or report the blocker explicitly; do not silently substitute weaker checks.
Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
            design = design_path.display(),
            agents = agents_path.display(),
        ),
        PhaseTask::Validate => format!(
            "\
Open {plan} and locate exactly:

## Phase {phase} - {title}

Validate that Phase {phase} was implemented fully and correctly in the current lnk repo.
Compare the current implementation against the selected phase's Goal, Steps, Validation, and Web validation sections.
Reinspect the files touched by the phase and the surrounding modules.
Close whatever gaps are needed to make Phase {phase} complete, correct, and well-verified.
Do not skip ahead into later phases except for a small prerequisite required to satisfy this phase.
Do not add commands, flags, configuration, workflows, CI, release automation, or features outside {design} and the selected phase.
Fix issues, strengthen tests or verification where needed, and keep the work focused on satisfying Phase {phase} end to end.
Run the checks required by {agents} and the selected phase. Fix failures.
If a required tool such as cargo-nextest is missing, install it when practical or report the blocker explicitly; do not silently substitute weaker checks.
Review git diff before finishing.
Do not commit or push unless the user explicitly requested it for this run.",
            plan = plan_path.display(),
            design = design_path.display(),
            agents = agents_path.display(),
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

        assert!(prompt.contains("## Phase 4 - Path And Environment Resolution"));
        assert!(prompt.contains("Treat DESIGN.md as the product contract"));
        assert!(prompt.contains("Do not commit or push"));
    }
}
