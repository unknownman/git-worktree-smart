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

    // Compare canonicalized paths against the true main repo root. This is
    // robust across symlinked filesystems and non-standard setups (e.g.
    // submodules or worktree index anomalies), unlike probing for a `.git`
    // directory.
    let repo_root = git::get_repo_root(ctx.verbose)?;
    let is_main_worktree = match (
        std::fs::canonicalize(&info.path),
        std::fs::canonicalize(&repo_root),
    ) {
        (Ok(info_canon), Ok(root_canon)) => info_canon == root_canon,
        // If either path cannot be canonicalized, fall back to a direct
        // comparison to avoid a false negative.
        _ => info.path == repo_root,
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
    }

    git::ops::remove_worktree(ctx.verbose, &info.path, force)?;

    if ctx.json {
        output::json::print_single(&info)?;
    } else {
        output::human::print_remove_success(&info, force);
    }

    Ok(())
}
