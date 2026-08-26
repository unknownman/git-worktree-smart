use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "wt",
    about = "A lightweight, human-friendly, and beautiful Git worktree manager",
    long_about = "git-worktree-smart (wt) — zero-config Git worktree management.\n\n\
        Manage Git worktrees without leaving your standard workflow.\n\
        No bare repositories. No complex setup. Just smarter worktrees.",
    version,
    propagate_version = true,
    after_help = "Run 'wt <command> --help' for more information on a specific command."
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
    ///
    /// Displays a table of every worktree, its linked branch, HEAD commit,
    /// and current status (dirty, ahead, behind).
    #[command(alias = "ls")]
    List,

    /// Create a new worktree for a branch.
    ///
    /// Automatically infers a sensible path outside the current working tree
    /// based on the branch name. If the branch does not yet exist, it will be
    /// created from the specified base (defaults to HEAD).
    #[command(after_help = "EXAMPLES:\n\
            wt add feature/auth            # new worktree on branch 'feature/auth'\n\
            wt add hotfix main             # new worktree branching from 'main'\n\
            wt add experiment HEAD~3       # new worktree from 3 commits ago")]
    Add {
        /// The name of the new branch and worktree.
        name: String,

        /// Starting point for the new branch (e.g., main, HEAD~3).
        ///
        /// If omitted, branches from the current HEAD.
        base: Option<String>,

        /// Set up tracking for a remote branch (e.g., origin/feature/auth).
        ///
        /// The branch must already exist on the remote.
        #[arg(long)]
        track: Option<String>,
    },

    /// Quickly switch to another worktree by name or path.
    ///
    /// Matches against branch names and path substrings to find the target.
    ///
    /// NOTE: Since a child process cannot change the parent shell's working
    /// directory, this command prints shell instructions for you to evaluate.
    /// For seamless integration, pair with a shell wrapper (planned for a
    /// future release).
    Switch {
        /// Branch name, path, or substring to match against.
        target: String,
    },

    /// Safely remove an existing worktree.
    ///
    /// Accepts either a branch name or an absolute/relative path.
    /// By default, refuses to remove worktrees with uncommitted changes
    /// or unpushed commits. Use --force to override.
    #[command(alias = "rm")]
    #[command(after_help = "EXAMPLES:\n\
            wt remove feature/auth          # safe removal (checks for dirty/unpushed)\n\
            wt rm feature/auth              # alias for 'remove'\n\
            wt remove --force feature/auth  # force removal (data loss possible)")]
    Remove {
        /// The branch name or path of the worktree to remove.
        target: String,

        /// Force removal even if the worktree has uncommitted changes
        /// or unpushed commits.
        ///
        /// WARNING: This can result in data loss. Use with caution.
        #[arg(short, long)]
        force: bool,
    },

    /// Clean up stale worktree references in the Git index.
    ///
    /// Wraps `git worktree prune` but defaults to a safe dry-run preview
    /// showing exactly what would be removed. Pass -y to execute.
    Prune {
        /// Skip the dry-run preview and execute the prune immediately.
        #[arg(short, long)]
        yes: bool,
    },

    /// Print the absolute path of a target worktree.
    ///
    /// Useful for scripting: cd $(wt path feature/auth)
    Path {
        /// Branch name or path substring to match against.
        target: String,
    },
}
