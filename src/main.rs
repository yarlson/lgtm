mod cli;
mod commands;

#[cfg(test)]
#[allow(dead_code)]
mod app_server;
#[cfg(test)]
#[allow(dead_code)]
mod output;

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
