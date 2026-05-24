#[derive(Clone, Debug)]
pub struct AppServerConfig {
    pub model: String,
    pub reasoning_effort: String,
    pub sandbox: String,
    pub approval_policy: String,
    pub developer_instructions: String,
    pub service_name: String,
    pub client_name: String,
    pub client_title: String,
    pub client_version: String,
}
