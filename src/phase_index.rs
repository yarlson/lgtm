use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::HashSet;

pub const PARSER_MODEL: &str = "gpt-5.4-mini";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phase {
    pub id: u32,
    pub title: String,
    pub heading: String,
}

pub fn parser_prompt(plan_path: &std::path::Path, plan: &str) -> String {
    format!(
        "\
Parse the following {plan_path} content into a strict JSON phase index.

Return only JSON with this exact shape:
{{\"phases\":[{{\"id\":1,\"title\":\"Skeleton\",\"heading\":\"## Phase 1 - Skeleton\"}}]}}

Rules:
- Include every implementation phase from PLAN.md.
- Use the numeric phase id from headings such as `## Phase 1 - Name` or `## Phase 1: Name`.
- Preserve each title text after the dash or colon.
- Preserve the exact markdown heading line in `heading`.
- Do not include markdown fences, commentary, or extra keys.

PLAN.md content:
```md
{plan}
```",
        plan_path = plan_path.display()
    )
}

pub fn repair_prompt(previous_output: &str) -> String {
    format!(
        "\
Your previous phase-index response was invalid.

Return only valid JSON with this exact shape:
{{\"phases\":[{{\"id\":1,\"title\":\"Skeleton\",\"heading\":\"## Phase 1 - Skeleton\"}}]}}

Previous output:
{previous_output}"
    )
}

pub fn parse_phase_index(output: &str) -> Result<Vec<Phase>> {
    let value: Value = serde_json::from_str(output.trim()).context("phase index was not JSON")?;
    let phases = value
        .get("phases")
        .and_then(Value::as_array)
        .context("phase index JSON must contain a phases array")?;

    let mut seen = HashSet::new();
    let mut parsed = Vec::with_capacity(phases.len());
    for phase in phases {
        let id = phase
            .get("id")
            .and_then(Value::as_u64)
            .context("phase index entry missing numeric id")?;
        let id = u32::try_from(id).context("phase id is too large")?;
        if id == 0 {
            bail!("phase id must be positive");
        }
        if !seen.insert(id) {
            bail!("phase id {id} appears more than once");
        }

        let title = phase
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .context("phase index entry missing non-empty title")?;
        let heading = phase
            .get("heading")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|heading| !heading.is_empty())
            .context("phase index entry missing non-empty heading")?;
        parsed.push(Phase {
            id,
            title: title.to_string(),
            heading: heading.to_string(),
        });
    }

    parsed.sort_by_key(|phase| phase.id);
    if parsed.is_empty() {
        bail!("phase index did not contain any phases");
    }
    Ok(parsed)
}

pub fn next_phase(phases: &[Phase], phase_id: u32, end_phase: u32) -> Option<Phase> {
    phases
        .iter()
        .find(|phase| phase.id >= phase_id && phase.id <= end_phase)
        .cloned()
}

pub fn detected_end_phase(phases: &[Phase], plan_path: &std::path::Path) -> Result<u32> {
    phases
        .iter()
        .map(|phase| phase.id)
        .max()
        .with_context(|| format!("could not detect end phase from {}", plan_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_strict_phase_json() {
        let phases =
            parse_phase_index(
                r###"{"phases":[{"id":2,"title":"Two","heading":"## Phase 2 - Two"},{"id":1,"title":"One","heading":"## Phase 1 - One"}]}"###,
            )
            .unwrap();

        assert_eq!(
            phases,
            vec![
                Phase {
                    id: 1,
                    title: "One".to_string(),
                    heading: "## Phase 1 - One".to_string()
                },
                Phase {
                    id: 2,
                    title: "Two".to_string(),
                    heading: "## Phase 2 - Two".to_string()
                }
            ]
        );
    }

    #[test]
    fn rejects_duplicate_phase_ids() {
        let error =
            parse_phase_index(
                r###"{"phases":[{"id":1,"title":"One","heading":"## Phase 1 - One"},{"id":1,"title":"Again","heading":"## Phase 1 - Again"}]}"###,
            )
            .unwrap_err();

        assert!(error.to_string().contains("appears more than once"));
    }
}
