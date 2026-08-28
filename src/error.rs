use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not a git repository (or any of the parent directories). Run `git init` first.")]
    NotAGitRepository,

    #[error("Git executable was not found in your PATH. Please ensure Git is installed.")]
    GitNotFound,

    #[error("`wt` must be run inside a working tree, not a bare repository.")]
    BareRepositoryNotSupported,

    #[error("Worktree at `{}` has uncommitted changes. 💡 Pass --force to delete it anyway.", .path.display())]
    WorktreeIsDirty { path: PathBuf },

    #[error(
        "Worktree at `{}` has {ahead} unmerged or unpushed commit(s). 💡 Pass --force to delete it anyway.",
        .path.display()
    )]
    UnpushedCommits { path: PathBuf, ahead: u32 },

    #[error("Cannot remove the main worktree at `{}` — it is the root of the repository.", .path.display())]
    CannotRemoveMainWorktree { path: PathBuf },

    #[error("Cannot remove the active worktree you are currently in (`{}`). Switch to another worktree first: `cd $(wt path main)`", .path.display())]
    CannotRemoveActiveWorktree { path: PathBuf },

    #[error(
        "Worktree directory at `{}` is missing. Use `wt prune` to clean up stale references.",
        .path.display()
    )]
    StaleWorktree { path: PathBuf },

    #[error("Worktree at `{}` is in a detached HEAD state with commits not reachable from any branch. 💡 Pass --force to delete it anyway.", .path.display())]
    DetachedHeadWithUnreachableCommits { path: PathBuf },

    #[error("Git failed: {message}")]
    GitError { message: String },

    #[error("JSON serialization failed: {message}")]
    JsonError { message: String },

    #[error("Failed to infer worktree path: {reason}")]
    PathInferenceFailed { reason: String },

    #[error("No worktree found for query `{query}`.")]
    WorktreeNotFound { query: String },

    #[error(
        "Multiple worktrees match `{query}`. Candidates are: {}",
        .candidates.join(", ")
    )]
    MultipleWorktreesMatch {
        query: String,
        candidates: Vec<String>,
    },

    #[error("Branch `{branch}` already exists locally. Cannot specify `--base` or `--track` when checking out an existing branch.")]
    BranchAlreadyExistsCannotSpecifyBaseOrTrack { branch: String },

    #[error("Branch `{branch}` is already checked out at `{}`.", .path.display())]
    BranchAlreadyCheckedOut { branch: String, path: PathBuf },

    #[error("Cannot create worktree: a file or directory already exists at `{}`. 💡 If this is a leftover from a deleted worktree, run `wt prune` to clean it up first.", .path.display())]
    PathAlreadyExists { path: PathBuf },

    #[error("Cannot create worktree in an empty repository. Please make an initial commit first.")]
    EmptyRepository,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
