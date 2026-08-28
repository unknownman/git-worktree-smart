use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "wt",
    about = "wt - A lightweight, zero-config Git worktree manager.",
    long_about = "wt — a lightweight, zero-config Git worktree manager.\n\n\
        Manage Git worktrees without leaving your standard workflow.\n\
        No bare repositories. No complex setup. Just smarter worktrees.\n\
        Smart path inference, fuzzy matching, and safe-by-default destructive\n\
        operations built right in.",
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
    /// and current status (dirty, ahead, behind). This is the default command
    /// when no subcommand is given.
    #[command(
        alias = "ls",
        after_help = "EXAMPLES:\n\
            wt list               # show every worktree\n\
            wt ls                 # alias for 'list'\n\
            wt list --json        # machine-readable output"
    )]
    List,

    /// Create a new worktree for a branch.
    ///
    /// Automatically infers a sensible path outside the current working tree
    /// based on the branch name (e.g. `feature/auth` becomes `../repo-feature-auth`).
    /// If the branch does not yet exist, it will be created from the specified
    /// base (defaults to HEAD).
    #[command(after_help = "EXAMPLES:\n\
            wt add feature/auth                     # new worktree on branch 'feature/auth'\n\
            wt add feature/auth main                # branch from 'main'\n\
            wt add hotfix --track origin/hotfix     # track an existing remote branch\n\
            wt add feature/auth --path ../custom    # worktree at a custom path")]
    Add {
        /// The name of the new branch and worktree.
        name: String,

        /// The starting point for the new branch (e.g., main, HEAD~3).
        ///
        /// Defaults to the current HEAD if omitted. Ignored when the branch
        /// already exists.
        base: Option<String>,

        /// Set up tracking for a remote branch (e.g., origin/feature/auth).
        ///
        /// The branch must already exist on the remote.
        #[arg(long, conflicts_with = "base")]
        track: Option<String>,

        /// Create the worktree at this custom path instead of the inferred
        /// sibling path. Relative paths are resolved against the current
        /// working directory.
        #[arg(short, long)]
        path: Option<PathBuf>,
    },

    /// Resolve a target worktree and print the path to switch to.
    ///
    /// Matches worktrees via exact, substring, or fuzzy matching. Because a
    /// child process cannot change the parent shell's directory, pair this with
    /// the path command instead: `cd $(wt path feature/auth)`.
    #[command(
        alias = "cd",
        after_help = "SHELL INTEGRATION:\n\
            A child process cannot change the parent shell's directory, so pair\n\
            switch with the path command: `cd $(wt path feature/auth)`.\n\
            For instant switching, add this wrapper to ~/.zshrc or ~/.bashrc:\n\n\
            wt() {\n\
                local has_json=false\n\
                for arg in \"$@\"; do\n\
                    if [ \"$arg\" = \"--json\" ]; then has_json=true; fi\n\
                done\n\n\
                local is_switch=false\n\
                if [ \"$1\" = \"switch\" ] || [ \"$1\" = \"cd\" ]; then\n\
                    is_switch=true\n\
                fi\n\n\
                if [ \"$is_switch\" = true ] && [ \"$has_json\" = false ]; then\n\
                    shift\n\
                    local target_path\n\
                    target_path=\"$(command wt path \"$@\")\"\n\
                    if [ $? -eq 0 ] && [ -n \"$target_path\" ]; then\n\
                        cd -- \"$target_path\"\n\
                    fi\n\
                else\n\
                    command wt \"$@\"\n\
                fi\n\
            }\n\n\
            EXAMPLES:\n\
            wt switch login           # fuzzy-match 'feature/login'\n\
            wt switch feature/login   # exact branch match\n\
            wt cd main                # alias for 'switch'"
    )]
    Switch {
        /// Branch name, path, or substring to match against.
        ///
        /// Multiple words are joined with spaces to form a fuzzy query
        /// (e.g. `wt switch feature auth`).
        #[arg(num_args(1..))]
        target: Vec<String>,
    },

    /// Safely remove an existing worktree.
    ///
    /// Accepts a branch name, path, or a fuzzy match. By default, refuses to
    /// remove worktrees with uncommitted changes or unpushed commits. Use
    /// --force to override (data loss possible).
    #[command(
        alias = "rm",
        after_help = "EXAMPLES:\n\
            wt remove feature/auth          # safe removal (checks for dirty/unpushed)\n\
            wt rm feature/auth              # alias for 'remove'\n\
            wt remove --force feature/auth  # force removal (data loss possible)"
    )]
    Remove {
        /// The branch name, path, or substring of the worktree to remove.
        ///
        /// Multiple words are joined with spaces to form a fuzzy query
        /// (e.g. `wt remove feature auth`).
        #[arg(num_args(1..))]
        target: Vec<String>,

        /// Force removal even if the worktree has uncommitted changes or
        /// unpushed commits.
        ///
        /// WARNING: This can result in data loss. Use with caution.
        #[arg(short, long)]
        force: bool,
    },

    /// Clean up stale worktree references in the Git index.
    ///
    /// Wraps `git worktree prune` but defaults to a safe dry-run preview
    /// showing exactly what would be removed. Pass --yes to execute.
    #[command(after_help = "EXAMPLES:\n\
            wt prune              # dry-run: preview what would be removed\n\
            wt prune --yes        # actually remove stale references\n\
            wt prune --json       # machine-readable preview")]
    Prune {
        /// Skip the dry-run preview and execute the prune immediately.
        #[arg(short, long)]
        yes: bool,
    },

    /// Print the absolute path of a target worktree.
    ///
    /// Useful for scripting: `cd $(wt path feature/auth)`. Prints only the
    /// path to stdout, so it is safe to shell-evaluate.
    #[command(after_help = "EXAMPLES:\n\
            wt path login                  # fuzzy-match a worktree\n\
            cd $(wt path feature/auth)     # shell-eval to change directory")]
    Path {
        /// Branch name or path substring to match against.
        ///
        /// Multiple words are joined with spaces to form a fuzzy query
        /// (e.g. `wt path feature auth`).
        #[arg(num_args(1..))]
        target: Vec<String>,
    },
}
