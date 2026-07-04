use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

pub fn validate_plan_file(plan_path: &Path) -> Result<()> {
    let plan = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read plan {}", plan_path.display()))?;
    validate_plan_contract(plan_path, &plan)
}

pub fn validate_plan_contract(plan_path: &Path, plan: &str) -> Result<()> {
    let mut errors = Vec::new();

    if !plan.lines().any(|line| line.trim() == "# Plan") {
        errors.push("missing required `# Plan` heading".to_string());
    }

    for heading in [
        "## Decisions",
        "## Non-Goals",
        "## Open Risks",
        "## Loopholes To Close",
    ] {
        if !plan.lines().any(|line| line.trim() == heading) {
            errors.push(format!("missing required `{heading}` section"));
        }
    }

    let mut phase_count = 0;
    let mut current_phase: Option<PhaseContract> = None;
    for line in plan.lines() {
        let trimmed = line.trim();
        if is_phase_heading(trimmed) {
            if let Some(phase) = current_phase.take() {
                phase.validate(&mut errors);
            }
            phase_count += 1;
            current_phase = Some(PhaseContract::new(trimmed));
            continue;
        }

        if let Some(phase) = current_phase.as_mut() {
            phase.observe(line);
        }
    }

    if let Some(phase) = current_phase {
        phase.validate(&mut errors);
    }

    if phase_count == 0 {
        errors.push("does not contain any `## Phase N - Name` headings".to_string());
    }

    if !errors.is_empty() {
        bail!(
            "plan {} failed contract validation:\n- {}",
            plan_path.display(),
            errors.join("\n- ")
        );
    }

    Ok(())
}

struct PhaseContract {
    heading: String,
    has_goal: bool,
    has_deliverables: bool,
    has_dependencies: bool,
    has_unresolved_decisions: bool,
    has_steps: bool,
    has_validation: bool,
}

impl PhaseContract {
    fn new(heading: &str) -> Self {
        Self {
            heading: heading.to_string(),
            has_goal: false,
            has_deliverables: false,
            has_dependencies: false,
            has_unresolved_decisions: false,
            has_steps: false,
            has_validation: false,
        }
    }

    fn observe(&mut self, line: &str) {
        let trimmed = line.trim();
        self.has_goal |= trimmed == "Goal:";
        self.has_deliverables |= trimmed == "Deliverables:";
        self.has_dependencies |= trimmed == "Dependencies:";
        self.has_unresolved_decisions |= trimmed == "Unresolved decisions:";
        self.has_steps |= trimmed == "Steps:";
        self.has_validation |= trimmed == "Validation:";
    }

    fn validate(self, errors: &mut Vec<String>) {
        for (label, found) in [
            ("Goal:", self.has_goal),
            ("Deliverables:", self.has_deliverables),
            ("Dependencies:", self.has_dependencies),
            ("Unresolved decisions:", self.has_unresolved_decisions),
            ("Steps:", self.has_steps),
            ("Validation:", self.has_validation),
        ] {
            if !found {
                errors.push(format!(
                    "phase `{}` is missing required `{}` label",
                    self.heading, label
                ));
            }
        }
    }
}

fn is_phase_heading(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("## Phase ") else {
        return false;
    };
    let Some((number, name)) = rest.split_once(" - ") else {
        return false;
    };

    !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) && !name.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_PLAN: &str = "\
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
    fn accepts_valid_plan_contract() {
        validate_plan_contract(Path::new("PLAN.md"), VALID_PLAN).expect("contract");
    }

    #[test]
    fn rejects_plan_contract_missing_plan_heading() {
        let error = validate_plan_contract(
            Path::new("PLAN.md"),
            "## Phase 1 - Test\n\nGoal:\nShip.\n\nDeliverables:\n- D.\n\nDependencies:\n- None.\n\nUnresolved decisions:\n- None.\n\nSteps:\n- Do it.\n\nValidation:\n- Check it.\n",
        )
        .expect_err("missing plan heading");

        assert!(error.to_string().contains("missing required `# Plan`"));
    }

    #[test]
    fn rejects_plan_contract_missing_decision_sections() {
        let error = validate_plan_contract(
            Path::new("PLAN.md"),
            "# Plan\n\n## Phase 1 - Test\n\nGoal:\nShip.\n\nDeliverables:\n- D.\n\nDependencies:\n- None.\n\nUnresolved decisions:\n- None.\n\nSteps:\n- Do it.\n\nValidation:\n- Check it.\n",
        )
        .expect_err("missing decisions");

        assert!(
            error
                .to_string()
                .contains("missing required `## Decisions` section")
        );
        assert!(
            error
                .to_string()
                .contains("missing required `## Non-Goals` section")
        );
    }

    #[test]
    fn rejects_plan_contract_missing_phase_heading() {
        let error = validate_plan_contract(
            Path::new("PLAN.md"),
            "# Plan\n\n## Decisions\n\n- D.\n\n## Non-Goals\n\n- N.\n\n## Open Risks\n\n- R.\n\n## Loopholes To Close\n\n- L.\n\nGoal:\nShip.\n\nDeliverables:\n- D.\n\nDependencies:\n- None.\n\nUnresolved decisions:\n- None.\n\nSteps:\n- Do it.\n\nValidation:\n- Check it.\n",
        )
        .expect_err("missing phase heading");

        assert!(
            error
                .to_string()
                .contains("does not contain any `## Phase N - Name`")
        );
    }

    #[test]
    fn rejects_plan_contract_missing_required_block_label() {
        let error = validate_plan_contract(
            Path::new("PLAN.md"),
            "# Plan\n\n## Decisions\n\n- D.\n\n## Non-Goals\n\n- N.\n\n## Open Risks\n\n- R.\n\n## Loopholes To Close\n\n- L.\n\n## Phase 1 - Test\n\nGoal:\nShip.\n\nValidation:\n- Check it.\n",
        )
        .expect_err("missing label");

        assert!(
            error
                .to_string()
                .contains("missing required `Deliverables:`")
        );
        assert!(error.to_string().contains("missing required `Steps:`"));
    }

    #[test]
    fn rejects_phase_labels_embedded_in_prose() {
        let error = validate_plan_contract(
            Path::new("PLAN.md"),
            "# Plan\n\n## Decisions\n\n- D.\n\n## Non-Goals\n\n- N.\n\n## Open Risks\n\n- R.\n\n## Loopholes To Close\n\n- L.\n\n## Phase 1 - Test\n\nGoal: Ship.\n\nDeliverables: D.\n\nDependencies: None.\n\nUnresolved decisions: None.\n\nSteps: Do it.\n\nValidation: Check it.\n",
        )
        .expect_err("embedded labels");

        assert!(error.to_string().contains("missing required `Goal:`"));
        assert!(
            error
                .to_string()
                .contains("missing required `Deliverables:`")
        );
        assert!(error.to_string().contains("missing required `Validation:`"));
    }
}
