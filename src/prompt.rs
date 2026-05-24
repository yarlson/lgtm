use crate::phase_index::Phase;

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

fn context_docs_block(plan_path: &std::path::Path, agents_path: &std::path::Path) -> String {
    format!(
        "\
Read these context files before coding:
- {agents}
- {plan}
Treat {plan} as the implementation order and the source of any linked project-contract context.
Treat {agents} as the authoritative agent instructions for this run.
Use only repo-local files and official docs relevant to the selected phase.
When validating version-sensitive Rust, Cargo, Git, dependency, or test-runner behavior, check the installed version and repo-local config first; use current official docs when local evidence is not enough.",
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
Use $lgtm-plan-update only if PLAN.md needs a correction to make this selected phase implementable or verifiable.
Use $lgtm-spec-update only if the selected phase exposes a real product or architecture contract gap that belongs in PLAN.md or a doc linked from PLAN.md.

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
Use $lgtm-docs-drift-review if changed behavior may affect README, AGENTS.md, PLAN.md, linked project docs, API docs, operational docs, examples, or command help.
Use $lgtm-rollout-review if the phase affects deployment, infrastructure, runtime config, migrations, observability, rollback, or production failure modes.
Use $lgtm-dependency-review if the phase changes dependencies, lockfiles, package manager config, generated files, CI security config, tool versions, or plugin/MCP/tool installation.

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
}
