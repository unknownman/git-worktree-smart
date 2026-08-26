use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "wt",
    about = "A lightweight, human-friendly, and beautiful Git worktree manager",
    version,
    propagate_version = true
)]
pub struct Cli {
    /// Output results as JSON for machine consumption.
    #[arg(long, global = true)]
    pub json: bool,

    /// Enable verbose output showing the underlying git commands being executed.
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all worktrees in the current repository.
    List,

    /// Create a new worktree for a branch.
    ///
    /// Automatically infers a sensible path outside the current working tree
    /// based on the branch name. If the branch does not yet exist, it will be
    /// created from the current HEAD.
    Add {
        /// The branch name to associate with the new worktree.
        /// If the branch does not exist, it will be created.
        branch: String,
    },

    /// Safely remove an existing worktree.
    ///
    /// Accepts either a branch name or an absolute/relative path to the
    /// worktree. Fails safely if the worktree has uncommitted changes or
    /// unpushed commits.
    Rm {
        /// The branch name or path of the worktree to remove.
        target: String,

        /// Force removal even if the worktree has uncommitted changes.
        ///
        /// WARNING: This can result in data loss.
        #[arg(short, long)]
        force: bool,
    },

    /// Prune stale worktree administrative files.
    ///
    /// Wraps `git worktree prune` but shows a dry-run preview by default
    /// so you can see what would be removed before committing.
    Clean,
}
