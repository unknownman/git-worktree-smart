use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not a git repository (or any of the parent directories)")]
    NotAGitRepository,

    #[error("worktree at `{path}` has uncommitted changes")]
    WorktreeIsDirty { path: PathBuf },

    #[error("worktree at `{path}` has unpushed commits")]
    WorktreeHasUnpushedCommits { path: PathBuf },

    #[error("failed to execute git: {message}")]
    GitError { message: String },

    #[error("failed to resolve worktree path: {reason}")]
    PathInferenceFailed { reason: String },

    #[error("worktree not found for query `{query}`")]
    WorktreeNotFound { query: String },

    #[error("multiple worktrees match query `{query}`, please be more specific")]
    MultipleWorktreesMatch { query: String },

    #[error("branch `{branch}` already has a linked worktree")]
    BranchAlreadyLinked { branch: String },

    #[error("worktree already exists at `{path}`")]
    WorktreeAlreadyExists { path: PathBuf },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
