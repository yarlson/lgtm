#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppServerLaunch {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
}

impl AppServerLaunch {
    pub fn new(program: impl Into<String>, args: impl IntoIterator<Item = String>) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().collect(),
            envs: Vec::new(),
        }
    }

    pub fn host(codex_bin: impl Into<String>) -> Self {
        Self::new(codex_bin, ["app-server".to_string()])
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.push((key.into(), value.into()));
        self
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn envs(&self) -> &[(String, String)] {
        &self.envs
    }

    pub fn display_command(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_launch_runs_codex_app_server() {
        let launch = AppServerLaunch::host("codex-test");

        assert_eq!(launch.program(), "codex-test");
        assert_eq!(launch.args(), ["app-server"]);
        assert!(launch.envs().is_empty());
    }

    #[test]
    fn launch_can_set_child_environment() {
        let launch = AppServerLaunch::host("codex-test").with_env("CODEX_HOME", "/tmp/codex");

        assert_eq!(
            launch.envs(),
            &[("CODEX_HOME".to_string(), "/tmp/codex".to_string())]
        );
    }
}
