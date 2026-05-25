const DEFAULT_DEVELOPER_INSTRUCTIONS: &str = "You are running inside lgtm. Follow the user prompt exactly and keep all work scoped to the current turn.";

#[derive(Clone, Debug)]
pub struct AppServerConfig {
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
    pub fn for_run(cwd: impl Into<String>, model: Option<String>) -> Self {
        Self {
            cwd: cwd.into(),
            model,
            reasoning_effort: "high".to_string(),
            sandbox: "danger-full-access".to_string(),
            approval_policy: "never".to_string(),
            developer_instructions: DEFAULT_DEVELOPER_INSTRUCTIONS.to_string(),
            service_name: "lgtm".to_string(),
            client_name: "lgtm".to_string(),
            client_title: "lgtm".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn with_developer_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.developer_instructions = instructions.into();
        self
    }
}
