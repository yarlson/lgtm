use std::path::Path;

#[derive(Clone, Debug)]
pub struct AppServerConfig {
    pub codex_bin: String,
    pub cwd: String,
    pub model: Option<String>,
    pub reasoning_effort: String,
    pub sandbox: String,
    pub approval_policy: String,
    pub developer_instructions: String,
    pub service_name: String,
    pub client_name: String,
    pub client_title: String,
    pub client_version: String,
}

impl AppServerConfig {
    pub fn for_run(codex_bin: impl Into<String>, root: &Path, model: Option<String>) -> Self {
        Self {
            codex_bin: codex_bin.into(),
            cwd: root.display().to_string(),
            model,
            reasoning_effort: "high".to_string(),
            sandbox: "danger-full-access".to_string(),
            approval_policy: "never".to_string(),
            developer_instructions: "You are running inside lgtm-rs. Follow the user prompt exactly and keep all work scoped to the current turn.".to_string(),
            service_name: "lgtm-rs".to_string(),
            client_name: "lgtm-rs".to_string(),
            client_title: "lgtm-rs".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
