use anyhow::{Context, Result, anyhow};
use serde_json::Value;

pub(crate) fn response_result(message: Value) -> Result<Value> {
    if let Some(error) = message.get("error") {
        let error_message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown JSON-RPC error");
        return Err(anyhow!("{error_message}"));
    }

    message
        .get("result")
        .cloned()
        .context("JSON-RPC response had neither result nor error")
}

pub(crate) fn get_string(value: &Value, path: &[&str]) -> Result<String> {
    get_str(value, path)
        .map(ToString::to_string)
        .with_context(|| format!("missing string at {}", path.join(".")))
}

pub(crate) fn get_str<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

pub(crate) fn get_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn response_result_extracts_success_payload() {
        let result = response_result(json!({
            "id": 1,
            "result": { "thread": { "id": "thr_123" } }
        }))
        .unwrap();

        assert_eq!(get_str(&result, &["thread", "id"]), Some("thr_123"));
    }

    #[test]
    fn response_result_reports_rpc_errors() {
        let err = response_result(json!({
            "id": 1,
            "error": { "code": -32000, "message": "not initialized" }
        }))
        .unwrap_err();

        assert_eq!(err.to_string(), "not initialized");
    }
}
