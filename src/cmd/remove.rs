use crate::error::AppError;
use crate::git;
use crate::output;
use crate::Context;

pub fn run(ctx: &Context, target: &str, force: bool) -> Result<(), AppError> {
    let info = git::resolve_worktree(ctx, target)?;

    // A manually-deleted worktree directory cannot be removed by `git worktree
    // remove`; point the user at `wt prune` to clean up the stale reference.
    if !info.path.exists() {
        return Err(AppError::StaleWorktree { path: info.path });
    }

    // Compare canonicalized paths against the true main worktree root. This is
    // robust across symlinked filesystems and non-standard setups (e.g.
    // submodules or worktree index anomalies), unlike probing for a `.git`
    // directory. The main worktree root is derived from `git worktree list`,
    // which is reliable even in submodules (`core.worktree`) where `.git` may
    // be a file rather than a directory.
    let main_root = git::get_main_worktree_root(ctx.verbose)?;
    let is_main_worktree = match (
        std::fs::canonicalize(&info.path),
        std::fs::canonicalize(&main_root),
    ) {
        (Ok(info_canon), Ok(root_canon)) => info_canon == root_canon,
        // If either path cannot be canonicalized, fall back to a direct
        // comparison to avoid a false negative.
        _ => info.path == main_root,
    };
    if is_main_worktree {
        return Err(AppError::CannotRemoveMainWorktree { path: info.path });
    }

    // Never allow removing the worktree you are currently standing in, even
    // with --force: it breaks the shell, locks the directory on Windows, and
    // triggers confusing Git errors.
    let active = std::env::current_dir()
        .ok()
        .and_then(|cwd| std::fs::canonicalize(&cwd).ok())
        .and_then(|cwd_canon| {
            std::fs::canonicalize(&info.path)
                .ok()
                .map(|p| (cwd_canon, p))
        })
        .filter(|(cwd_canon, path_canon)| cwd_canon.starts_with(path_canon))
        .is_some();
    if active {
        return Err(AppError::CannotRemoveActiveWorktree { path: info.path });
    }

    if !force {
        if info.status.is_dirty {
            return Err(AppError::WorktreeIsDirty { path: info.path });
        }
        if info.status.ahead > 0 {
            return Err(AppError::UnpushedCommits {
                path: info.path,
                ahead: info.status.ahead,
            });
        }
        // A detached HEAD worktree has no upstream to measure `ahead` against,
        // so commits made here can silently orphan. Guard against data loss by
        // verifying the commit is still reachable from some branch.
        if info.branch.is_none()
            && !git::is_commit_merged_or_reachable(&info.head_hash, &info.path, ctx.verbose)?
        {
            return Err(AppError::DetachedHeadWithUnreachableCommits { path: info.path });
        }
    }

    git::ops::remove_worktree(ctx.verbose, &info.path, force)?;

    if ctx.json {
        output::json::print_single(&info)?;
    } else {
        output::human::print_remove_success(&info, force);
    }

    Ok(())
}
