mod cli;
mod cmd;
mod error;
mod git;
mod models;
mod output;

use clap::Parser;
use cli::{Cli, Commands};

pub struct Context {
    pub json: bool,
    pub verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let ctx = Context {
        json: cli.json,
        verbose: cli.verbose,
    };

    match cli.command {
        Some(Commands::List) | None => cmd::list::run(&ctx),
        Some(Commands::Add { branch }) => cmd::add::run(&ctx, &branch),
        Some(Commands::Rm { target, force }) => cmd::rm::run(&ctx, &target, force),
        Some(Commands::Clean) => cmd::clean::run(&ctx),
    }
}
