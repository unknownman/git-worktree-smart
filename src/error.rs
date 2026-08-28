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

    #[error("Worktree at `{path}` has uncommitted changes. 💡 Pass --force to delete it anyway.")]
    WorktreeIsDirty { path: PathBuf },

    #[error(
        "Worktree at `{path}` has {ahead} unpushed commit(s). 💡 Pass --force to delete it anyway."
    )]
    UnpushedCommits { path: PathBuf, ahead: u32 },

    #[error("Cannot remove the main worktree at `{path}` — it is the root of the repository.")]
    CannotRemoveMainWorktree { path: PathBuf },

    #[error("Cannot remove the active worktree you are currently in (`{path}`). Switch to another worktree first: `cd $(wt path main)`")]
    CannotRemoveActiveWorktree { path: PathBuf },

    #[error(
        "Worktree directory at `{path}` is missing. Use `wt prune` to clean up stale references."
    )]
    StaleWorktree { path: PathBuf },

    #[error("Worktree at `{path}` is in a detached HEAD state with commits not reachable from any branch. 💡 Pass --force to delete it anyway.")]
    DetachedHeadWithUnreachableCommits { path: PathBuf },

    #[error("Git failed: {message}")]
    GitError { message: String },

    #[error("JSON serialization failed: {message}")]
    JsonError { message: String },

    #[error("Failed to infer worktree path: {reason}")]
    PathInferenceFailed { reason: String },

    #[error("No worktree found for query `{query}`.")]
    WorktreeNotFound { query: String },

    #[error("Multiple worktrees match `{query}`. Please be more specific.")]
    MultipleWorktreesMatch { query: String },

    #[error("Branch `{branch}` already exists locally. Cannot specify `--base` or `--track` when checking out an existing branch.")]
    BranchAlreadyExistsCannotSpecifyBaseOrTrack { branch: String },

    #[error("Branch `{branch}` is already checked out at `{path}`.")]
    BranchAlreadyCheckedOut { branch: String, path: PathBuf },

    #[error("Cannot create worktree: a file or directory already exists at `{path}`.")]
    PathAlreadyExists { path: PathBuf },

    #[error("Cannot create worktree in an empty repository. Please make an initial commit first.")]
    EmptyRepository,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
