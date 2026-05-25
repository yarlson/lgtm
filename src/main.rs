mod app_server;
mod cli;
mod commands;
mod composer;
mod git;
mod output;
mod paths;
mod phase_index;
mod prompt;
mod skills;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Plan(args) => commands::plan::run(args),
        Command::Run(args) => commands::run::run(args),
    }
}
