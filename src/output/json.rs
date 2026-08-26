use std::io::Write;

use crate::error::AppError;
use crate::models::WorktreeInfo;

pub fn print_list(worktrees: &[WorktreeInfo]) -> Result<(), AppError> {
    let json = serde_json::to_string_pretty(worktrees).map_err(|e| AppError::GitError {
        message: e.to_string(),
    })?;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{json}").map_err(|e| AppError::GitError {
        message: e.to_string(),
    })?;

    Ok(())
}

pub fn print_single(worktree: &WorktreeInfo) -> Result<(), AppError> {
    let json = serde_json::to_string_pretty(worktree).map_err(|e| AppError::GitError {
        message: e.to_string(),
    })?;

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{json}").map_err(|e| AppError::GitError {
        message: e.to_string(),
    })?;

    Ok(())
}
