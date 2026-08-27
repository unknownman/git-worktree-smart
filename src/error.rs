use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not a git repository (or any of the parent directories). Run `git init` first.")]
    NotAGitRepository,

    #[error("Worktree at `{path}` has uncommitted changes. 💡 Pass --force to delete it anyway.")]
    WorktreeIsDirty { path: PathBuf },

    #[error(
        "Worktree at `{path}` has {ahead} unpushed commit(s). 💡 Pass --force to delete it anyway."
    )]
    UnpushedCommits { path: PathBuf, ahead: u32 },

    #[error("Cannot remove the main worktree at `{path}` — it is the root of the repository.")]
    CannotRemoveMainWorktree { path: PathBuf },

    #[error("Git failed: {message}")]
    GitError { message: String },

    #[error("Failed to infer worktree path: {reason}")]
    PathInferenceFailed { reason: String },

    #[error("No worktree found for query `{query}`.")]
    WorktreeNotFound { query: String },

    #[error("Multiple worktrees match `{query}`. Please be more specific.")]
    MultipleWorktreesMatch { query: String },

    #[error("Branch `{branch}` already has a linked worktree.")]
    BranchAlreadyLinked { branch: String },

    #[error("A worktree already exists at `{path}`.")]
    WorktreeAlreadyExists { path: PathBuf },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
