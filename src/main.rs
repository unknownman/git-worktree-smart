mod cli;
mod cmd;
mod error;
mod git;
mod models;
mod output;

use std::process::ExitCode;

use clap::Parser;
use cli::{Cli, Commands};
use error::AppError;
use owo_colors::OwoColorize;

pub struct Context {
    pub json: bool,
    pub verbose: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let ctx = Context {
        json: cli.json,
        verbose: cli.verbose,
    };

    match dispatch(&cli, &ctx) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            render_error(&ctx, &err);
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: &Cli, ctx: &Context) -> Result<(), AppError> {
    match &cli.command {
        Some(Commands::List) | None => cmd::list::run(ctx),
        Some(Commands::Add {
            name,
            base,
            track,
            path,
        }) => cmd::add::run(
            ctx,
            name,
            base.as_deref(),
            track.as_deref(),
            path.as_deref(),
        ),
        Some(Commands::Switch { target }) => cmd::switch::run(ctx, &target.join(" ")),
        Some(Commands::Remove { target, force }) => {
            cmd::remove::run(ctx, &target.join(" "), *force)
        }
        Some(Commands::Prune { yes }) => cmd::clean::run(ctx, *yes),
        Some(Commands::Path { target }) => cmd::path::run(ctx, &target.join(" ")),
    }
}

fn render_error(ctx: &Context, err: &AppError) {
    if ctx.json {
        let obj = serde_json::json!({ "error": err.to_string() });
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&obj)
                .unwrap_or_else(|_| r#"{"error":"unknown"}"#.to_owned())
        );
    } else {
        eprintln!("{} {}", "Error:".bold().red(), err);
    }
}
