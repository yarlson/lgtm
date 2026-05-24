use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "Run Codex-backed LGTM workflows")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Plan(PlanArgs),
    Run(RunArgs),
}

#[derive(Debug, Args)]
pub struct PlanArgs {}

#[derive(Debug, Args)]
pub struct RunArgs {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plan_command() {
        let cli = Cli::try_parse_from(["lgtm-rs", "plan"]).unwrap();

        assert!(matches!(cli.command, Command::Plan(_)));
    }

    #[test]
    fn parses_run_command() {
        let cli = Cli::try_parse_from(["lgtm-rs", "run"]).unwrap();

        assert!(matches!(cli.command, Command::Run(_)));
    }
}
