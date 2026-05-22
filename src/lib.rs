mod cli;
mod codex;
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
    let config = cli::Config::from_args(args)?;
    codex::run_plan(config)
}
