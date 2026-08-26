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
        Some(Commands::List) | None => cmd::list::run(&ctx).map_err(Into::into),
        Some(Commands::Add { name, base, track }) => {
            cmd::add::run(&ctx, &name, base.as_deref(), track.as_deref())
                .map_err(Into::into)
        }
        Some(Commands::Switch { target }) => cmd::switch::run(&ctx, &target),
        Some(Commands::Remove { target, force }) => cmd::remove::run(&ctx, &target, force),
        Some(Commands::Prune { yes }) => cmd::clean::run(&ctx, yes),
        Some(Commands::Path { target }) => cmd::path::run(&ctx, &target),
    }
}
