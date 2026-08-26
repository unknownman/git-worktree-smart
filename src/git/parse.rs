use std::collections::HashMap;
use std::path::Path;

use crate::error::AppError;
use crate::git::command::run_git;
use crate::models::{Worktree, WorktreeStatus};

pub fn get_worktrees(verbose: bool) -> Result<Vec<Worktree>, AppError> {
    let output = run_git(&["worktree", "list", "--porcelain"], None, verbose)?;

    let mut worktrees = Vec::new();
    let mut current: HashMap<String, String> = HashMap::new();

    for line in output.lines() {
        if line.is_empty() {
            if let Some(wt) = flush_block(&current) {
                worktrees.push(wt);
            }
            current.clear();
        } else {
            if let Some((key, value)) = line.split_once(' ') {
                current.insert(key.to_owned(), value.to_owned());
            } else {
                current.insert(line.to_owned(), String::new());
            }
        }
    }

    if let Some(wt) = flush_block(&current) {
        worktrees.push(wt);
    }

    Ok(worktrees)
}

fn flush_block(block: &HashMap<String, String>) -> Option<Worktree> {
    let path = block.get("worktree")?.into();
    let head = block.get("HEAD").cloned().unwrap_or_default();
    let branch = block
        .get("branch")
        .map(|b| b.strip_prefix("refs/heads/").unwrap_or(b).to_owned());

    Some(Worktree {
        path,
        head,
        branch,
        status: WorktreeStatus::clean(),
    })
}

pub fn get_worktree_status(cwd: &Path, verbose: bool) -> Result<WorktreeStatus, AppError> {
    let dirty = is_dirty(cwd, verbose)?;
    let (ahead, behind) = get_ahead_behind(cwd, verbose)?;

    Ok(WorktreeStatus {
        dirty,
        ahead,
        behind,
    })
}

fn is_dirty(cwd: &Path, verbose: bool) -> Result<bool, AppError> {
    let output = run_git(&["status", "--porcelain"], Some(cwd), verbose)?;
    Ok(!output.is_empty())
}

fn get_ahead_behind(cwd: &Path, verbose: bool) -> Result<(u32, u32), AppError> {
    // Verify an upstream tracking branch exists before attempting the rev-list count.
    let upstream_check = run_git(
        &["rev-parse", "--verify", "--quiet", "@{u}"],
        Some(cwd),
        verbose,
    );

    if upstream_check.is_err() {
        return Ok((0, 0));
    }

    let output = run_git(
        &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
        Some(cwd),
        verbose,
    )?;

    let parts: Vec<&str> = output.split_whitespace().collect();
    if parts.len() < 2 {
        return Ok((0, 0));
    }

    let ahead = parts[0].parse::<u32>().unwrap_or(0);
    let behind = parts[1].parse::<u32>().unwrap_or(0);

    Ok((ahead, behind))
}
