mod cli;
mod codex;
mod composer;
mod error;
mod events;
mod git;
mod plan;
mod prompt;
mod render;
mod skills;
mod terminal;

use clap::Parser;

pub use error::Error;

pub fn run() -> Result<(), Error> {
    let args = cli::Args::parse();
    match args.command {
        cli::Command::Run(run_args) => {
            let config = cli::Config::from_args(run_args)?;
            codex::run_plan(config)
        }
        cli::Command::Plan(plan_args) => {
            let config = cli::PlanConfig::from_args(plan_args)?;
            codex::run_planning(config)
        }
    }
}
