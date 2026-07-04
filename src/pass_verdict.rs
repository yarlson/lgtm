use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

pub const VERDICT_MARKER: &str = "LGTM_VERDICT:";
pub const VERDICT_SCHEMA_VERSION: u64 = 1;

const VERDICT_KEYS: &[&str] = &[
    "schema_version",
    "status",
    "summary",
    "checks",
    "fixes",
    "blockers",
    "out_of_scope",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassVerdict {
    pub schema_version: u64,
    pub status: PassVerdictStatus,
    pub summary: String,
    pub checks: Vec<String>,
    pub fixes: Vec<String>,
    pub blockers: Vec<String>,
    pub out_of_scope: Vec<String>,
    pub raw_marker: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassVerdictStatus {
    Pass,
    Block,
}

pub fn parse_pass_verdict(response: &str) -> Result<PassVerdict> {
    let lines = response.lines().map(str::trim).collect::<Vec<_>>();
    let markers = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            line.strip_prefix(VERDICT_MARKER)
                .map(str::trim)
                .map(|marker| (index, marker))
        })
        .collect::<Vec<_>>();
    let Some((marker_index, verdict_json)) = markers.first().copied() else {
        bail!("missing {VERDICT_MARKER} marker")
    };
    if markers.len() > 1 {
        bail!("expected exactly one {VERDICT_MARKER} marker")
    }
    if lines
        .iter()
        .skip(marker_index + 1)
        .any(|line| !line.is_empty())
    {
        bail!("{VERDICT_MARKER} marker must be the final non-empty line")
    }
    if verdict_json.is_empty() {
        bail!("{VERDICT_MARKER} marker is empty")
    }

    let value: Value = serde_json::from_str(verdict_json).context("verdict marker was not JSON")?;
    let object = value
        .as_object()
        .context("verdict JSON must be an object")?;
    validate_exact_keys(object)?;
    let schema_version = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .context("verdict JSON missing numeric schema_version")?;
    if schema_version != VERDICT_SCHEMA_VERSION {
        bail!(
            "verdict schema_version must be {}, got {}",
            VERDICT_SCHEMA_VERSION,
            schema_version
        )
    }
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .map(str::trim)
        .context("verdict JSON missing string status")?;

    let summary = required_string_field(&value, "summary")?;
    let checks = string_array_field(&value, "checks")?;
    let fixes = string_array_field(&value, "fixes")?;
    let blockers = string_array_field(&value, "blockers")?;
    let out_of_scope = string_array_field(&value, "out_of_scope")?;

    match status {
        "pass" => {
            if checks.is_empty() {
                bail!("pass verdict must include at least one check")
            }
            if !blockers.is_empty() {
                bail!("pass verdict must not include blockers")
            }
            Ok(PassVerdict {
                schema_version,
                status: PassVerdictStatus::Pass,
                summary,
                checks,
                fixes,
                blockers,
                out_of_scope,
                raw_marker: verdict_json.to_string(),
            })
        }
        "block" => {
            if blockers.is_empty() {
                bail!("block verdict must include at least one blocker")
            }
            Ok(PassVerdict {
                schema_version,
                status: PassVerdictStatus::Block,
                summary,
                checks,
                fixes,
                blockers,
                out_of_scope,
                raw_marker: verdict_json.to_string(),
            })
        }
        other => bail!("verdict status must be `pass` or `block`, got `{other}`"),
    }
}

fn validate_exact_keys(object: &Map<String, Value>) -> Result<()> {
    let mut unknown = object
        .keys()
        .filter(|key| !VERDICT_KEYS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    unknown.sort();
    if !unknown.is_empty() {
        bail!("verdict JSON contains unknown keys: {}", unknown.join(", "))
    }

    let missing = VERDICT_KEYS
        .iter()
        .filter(|key| !object.contains_key(**key))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!("verdict JSON missing keys: {}", missing.join(", "))
    }

    Ok(())
}

fn required_string_field(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .with_context(|| format!("verdict `{field}` must be a non-empty string"))
}

fn string_array_field(value: &Value, field: &str) -> Result<Vec<String>> {
    let items = value
        .get(field)
        .with_context(|| format!("verdict missing `{field}` array"))?;
    let items = items
        .as_array()
        .with_context(|| format!("verdict `{field}` must be an array"))?;
    let mut parsed = Vec::with_capacity(items.len());
    for item in items {
        let item = item
            .as_str()
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .with_context(|| format!("verdict `{field}` entries must be non-empty strings"))?;
        parsed.push(item.to_string());
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pass_verdict_from_final_marker_line() {
        let verdict = parse_pass_verdict(
            "Validation passed.\nLGTM_VERDICT: {\"schema_version\":1,\"status\":\"pass\",\"summary\":\"validated\",\"checks\":[\"cargo test\"],\"fixes\":[],\"blockers\":[],\"out_of_scope\":[]}",
        )
        .expect("verdict");

        assert_eq!(verdict.status, PassVerdictStatus::Pass);
        assert_eq!(verdict.schema_version, VERDICT_SCHEMA_VERSION);
        assert_eq!(verdict.summary, "validated");
        assert_eq!(verdict.checks, ["cargo test"]);
    }

    #[test]
    fn parses_block_verdict_with_blockers() {
        let verdict = parse_pass_verdict(
            "Review blocked.\nLGTM_VERDICT: {\"schema_version\":1,\"status\":\"block\",\"summary\":\"blocked\",\"checks\":[],\"fixes\":[],\"blockers\":[\"missing test\"],\"out_of_scope\":[]}",
        )
        .expect("verdict");

        assert_eq!(verdict.status, PassVerdictStatus::Block);
        assert_eq!(verdict.blockers, ["missing test"]);
    }

    #[test]
    fn rejects_missing_verdict_marker() {
        let error = parse_pass_verdict("looks fine").expect_err("missing marker");

        assert!(error.to_string().contains("missing LGTM_VERDICT: marker"));
    }

    #[test]
    fn rejects_multiple_verdict_markers() {
        let response = "LGTM_VERDICT: {\"schema_version\":1,\"status\":\"pass\",\"summary\":\"ok\",\"checks\":[\"cargo test\"],\"fixes\":[],\"blockers\":[],\"out_of_scope\":[]}\nLGTM_VERDICT: {\"schema_version\":1,\"status\":\"pass\",\"summary\":\"ok\",\"checks\":[\"cargo test\"],\"fixes\":[],\"blockers\":[],\"out_of_scope\":[]}";

        let error = parse_pass_verdict(response).expect_err("multiple markers");

        assert!(
            error
                .to_string()
                .contains("expected exactly one LGTM_VERDICT: marker")
        );
    }

    #[test]
    fn rejects_verdict_marker_that_is_not_final_line() {
        let response = "LGTM_VERDICT: {\"schema_version\":1,\"status\":\"pass\",\"summary\":\"ok\",\"checks\":[\"cargo test\"],\"fixes\":[],\"blockers\":[],\"out_of_scope\":[]}\nmore prose";

        let error = parse_pass_verdict(response).expect_err("marker not final");

        assert!(
            error
                .to_string()
                .contains("LGTM_VERDICT: marker must be the final non-empty line")
        );
    }

    #[test]
    fn rejects_block_without_blockers() {
        let error = parse_pass_verdict(
            "LGTM_VERDICT: {\"schema_version\":1,\"status\":\"block\",\"summary\":\"blocked\",\"checks\":[],\"fixes\":[],\"blockers\":[],\"out_of_scope\":[]}",
        )
            .expect_err("missing blockers");

        assert!(
            error
                .to_string()
                .contains("must include at least one blocker")
        );
    }

    #[test]
    fn rejects_pass_without_checks() {
        let error = parse_pass_verdict(
            "LGTM_VERDICT: {\"schema_version\":1,\"status\":\"pass\",\"summary\":\"ok\",\"checks\":[],\"fixes\":[],\"blockers\":[],\"out_of_scope\":[]}",
        )
        .expect_err("missing checks");

        assert!(
            error
                .to_string()
                .contains("pass verdict must include at least one check")
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        let error = parse_pass_verdict(
            "LGTM_VERDICT: {\"schema_version\":1,\"status\":\"pass\",\"summary\":\"ok\",\"checks\":[\"cargo test\"],\"fixes\":[],\"blockers\":[],\"out_of_scope\":[],\"extra\":true}",
        )
        .expect_err("unknown key");

        assert!(error.to_string().contains("unknown keys: extra"));
    }

    #[test]
    fn rejects_missing_required_arrays() {
        let error = parse_pass_verdict(
            "LGTM_VERDICT: {\"schema_version\":1,\"status\":\"pass\",\"summary\":\"ok\",\"checks\":[\"cargo test\"],\"fixes\":[],\"blockers\":[]}",
        )
        .expect_err("missing key");

        assert!(error.to_string().contains("missing keys: out_of_scope"));
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let error = parse_pass_verdict(
            "LGTM_VERDICT: {\"schema_version\":2,\"status\":\"pass\",\"summary\":\"ok\",\"checks\":[\"cargo test\"],\"fixes\":[],\"blockers\":[],\"out_of_scope\":[]}",
        )
        .expect_err("schema");

        assert!(error.to_string().contains("schema_version must be 1"));
    }
}
