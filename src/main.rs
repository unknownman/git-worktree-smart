mod cli;
mod cmd;
mod error;
mod git;
mod models;
mod output;

use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::Parser;
use cli::{Cli, Commands};
use error::{AppError, CandidateMatch};
use output::human::shorten_home;
use owo_colors::OwoColorize;

pub struct Context {
    pub json: bool,
    pub verbose: bool,
}

fn main() -> ExitCode {
    // Use `try_parse` so an early CLI failure (e.g. `wt add --json` with a
    // missing branch name) can still honor the `--json` flag's promise of
    // strict, machine-readable output instead of raw human-readable stderr.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            // `--help` and `--version` are reported as errors by `try_parse`,
            // but they are not failures: print them and exit successfully.
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                err.print().unwrap();
                return ExitCode::SUCCESS;
            }

            // Other early failures (e.g. `wt add --json` with a missing branch)
            // must still honor `--json`'s promise of machine-readable output.
            let wants_json = std::env::args().any(|arg| arg == "--json");
            if wants_json {
                let error_msg = err.to_string();
                let error = error_msg.trim();
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({ "error": error }))
                        .unwrap_or_else(|_| r#"{"error":"unknown"}"#.to_owned())
                );
            } else {
                // Retain clap's default human-readable rendering (incl. usage).
                err.print().unwrap();
            }
            return ExitCode::from(2);
        }
    };

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
        Some(Commands::Prune { yes }) => cmd::prune::run(ctx, *yes),
        Some(Commands::Path { target }) => cmd::path::run(ctx, &target.join(" ")),
    }
}

fn render_error(ctx: &Context, err: &AppError) {
    if let AppError::MultipleWorktreesMatch { query, candidates } = err {
        if ctx.json {
            render_json_multiple_error(query, candidates);
        } else {
            render_human_multiple_error(query, candidates);
        }
        return;
    }

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

/// Render an ambiguous query in human-readable form, listing every matched
/// worktree (with `~`-shortened path) so the user knows how to disambiguate.
fn render_human_multiple_error(query: &str, candidates: &[CandidateMatch]) {
    eprintln!(
        "{} Multiple worktrees match `{}`.",
        "Error:".bold().red(),
        query.cyan()
    );
    eprintln!("{}", "Did you mean one of these?".bold());
    let max_name_len = candidates.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in candidates {
        let padding = " ".repeat(max_name_len.saturating_sub(c.name.len()) + 2);
        eprintln!(
            "  {} {}{}{}",
            "•".cyan().bold(),
            c.name.cyan(),
            padding,
            format!("({})", shorten_home(&c.path)).dimmed()
        );
    }
    eprintln!(
        "{} Be more specific or provide the exact branch name.",
        "💡 Tip:".yellow().bold()
    );
}

/// Render an ambiguous query as strict JSON, embedding a structured `candidates`
/// array so `--json` consumers can disambiguate programmatically.
fn render_json_multiple_error(query: &str, candidates: &[CandidateMatch]) {
    let obj = serde_json::json!({
        "error": format!("Multiple worktrees match `{query}`"),
        "candidates": candidates,
    });
    eprintln!(
        "{}",
        serde_json::to_string_pretty(&obj).unwrap_or_else(|_| r#"{"error":"unknown"}"#.to_owned())
    );
}
