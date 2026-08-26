use std::path::Path;

use crate::error::AppError;
use crate::git::command::run_git;

pub fn add_worktree(
    verbose: bool,
    path: &Path,
    branch_name: &str,
    base: Option<&str>,
    track: Option<&str>,
) -> Result<(), AppError> {
    let mut args = vec!["worktree", "add"];

    if let Some(upstream) = track {
        args.extend_from_slice(&["--track", upstream]);
    }

    if let Some(start) = base {
        args.extend_from_slice(&["-b", branch_name, start]);
    } else {
        args.extend_from_slice(&["-b", branch_name]);
    }

    let path_str = path.to_str().ok_or_else(|| AppError::PathInferenceFailed {
        reason: format!("path contains non-UTF-8 characters: {}", path.display()),
    })?;
    args.push(path_str);

    run_git(&args, None, verbose)?;
    Ok(())
}

pub fn remove_worktree(verbose: bool, path: &Path, force: bool) -> Result<(), AppError> {
    let mut args = vec!["worktree", "remove"];

    if force {
        args.push("--force");
    }

    let path_str = path.to_str().ok_or_else(|| AppError::PathInferenceFailed {
        reason: format!("path contains non-UTF-8 characters: {}", path.display()),
    })?;
    args.push(path_str);

    run_git(&args, None, verbose)?;
    Ok(())
}

pub fn prune_worktrees(verbose: bool) -> Result<(), AppError> {
    run_git(&["worktree", "prune"], None, verbose)?;
    Ok(())
}
