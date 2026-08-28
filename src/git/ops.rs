use std::path::Path;

use crate::error::AppError;
use crate::git::command::run_git;

pub fn add_worktree(
    verbose: bool,
    path: &Path,
    branch_name: &str,
    base: Option<&str>,
    track: Option<&str>,
    branch_exists: bool,
    remote_branch_exists: bool,
) -> Result<(), AppError> {
    let mut args = vec!["worktree", "add"];

    if branch_exists {
        // Check out an existing local branch: `git worktree add <path> <name>`.
        args.push(path_str(path)?);
        args.push(branch_name);
        run_git(&args, None, verbose)?;
        return Ok(());
    }

    // If the branch exists only on a remote, omit `-b` and let Git's default
    // retry-with-track (DWIM) behavior wire up the remote tracking
    // automatically: `git worktree add <path> <name>`.
    if remote_branch_exists && track.is_none() && base.is_none() {
        args.push(path_str(path)?);
        args.push(branch_name);
        run_git(&args, None, verbose)?;
        return Ok(());
    }

    // Create a new branch from an optional upstream or base.
    if let Some(upstream) = track {
        // syntax: git worktree add --track -b <branch> <path> <upstream>
        args.push("--track");
        args.extend_from_slice(&["-b", branch_name]);
        args.push(path_str(path)?);
        args.push(upstream);
    } else if let Some(start) = base {
        args.extend_from_slice(&["-b", branch_name]);
        args.push(path_str(path)?);
        args.push(start);
    } else {
        args.extend_from_slice(&["-b", branch_name]);
        args.push(path_str(path)?);
    }

    run_git(&args, None, verbose)?;
    Ok(())
}

fn path_str(path: &Path) -> Result<&str, AppError> {
    path.to_str().ok_or_else(|| AppError::PathInferenceFailed {
        reason: format!("path contains non-UTF-8 characters: {}", path.display()),
    })
}

pub fn remove_worktree(verbose: bool, path: &Path, force: bool) -> Result<(), AppError> {
    let mut args = vec!["worktree", "remove"];

    if force {
        args.push("--force");
    }

    args.push(path_str(path)?);

    run_git(&args, None, verbose)?;
    Ok(())
}

pub fn prune_worktrees(verbose: bool) -> Result<(), AppError> {
    run_git(&["worktree", "prune"], None, verbose)?;
    Ok(())
}
