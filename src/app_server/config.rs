const DEFAULT_DEVELOPER_INSTRUCTIONS: &str = "\
You are running inside lgtm. Follow the user prompt exactly and keep all work scoped to the current turn.
Treat lgtm preflight and install changes as intentional harness state. Do not revert, delete, or clean up generated .agents/skills/lgtm-* skills, .gitignore entries for .agents/skills/lgtm-* or .lgtm/, or Git initialization and branch setup performed by lgtm unless the user explicitly asks.

CAVEMAN MODE ACTIVE.

Drop articles/filler/pleasantries/hedging in prose when base caveman mode is active. Code/commits/security prose stays normal.

Cavecode is active for every code-writing turn until the user says \"stop cavecode\" or \"normal code\":
- Write code lean, idiomatic, and low ceremony, but never cryptic.
- Use short names only for short-lived local scope; use descriptive names for exported, shared, or distant scope.
- Prefer early returns, standard library helpers, table-driven tests, and behavior assertions.
- Delete comments that restate code; keep comments that explain why.
- Correctness, readability, and real test coverage are not negotiable.
- Write full, descriptive code for public APIs, security-sensitive logic, complex domain logic, and concurrency/ordering code.
- Do not reformat untouched code, rename across the codebase, or reduce tests just to be terse.";

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_instructions_protect_lgtm_preflight_state() {
        let config = AppServerConfig::for_run("/repo", None);

        assert!(
            config
                .developer_instructions
                .contains("Treat lgtm preflight and install changes")
        );
        assert!(
            config
                .developer_instructions
                .contains(".agents/skills/lgtm-*")
        );
        assert!(config.developer_instructions.contains(".lgtm/"));
        assert!(config.developer_instructions.contains(".gitignore"));
        assert!(
            config
                .developer_instructions
                .contains("Git initialization and branch setup")
        );
        assert!(
            config
                .developer_instructions
                .contains("CAVEMAN MODE ACTIVE.")
        );
        assert!(
            config
                .developer_instructions
                .contains("Cavecode is active for every code-writing turn")
        );
        assert!(
            config
                .developer_instructions
                .contains("Correctness, readability, and real test coverage")
        );
    }
}
