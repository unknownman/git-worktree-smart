use crate::error::AppError;
use crate::git;
use crate::output;
use crate::Context;

pub fn run(ctx: &Context, target: &str, force: bool) -> Result<(), AppError> {
    let info = git::resolve_worktree(ctx, target)?;

    // The main repository has a `.git` directory; linked worktrees have a
    // `.git` file. This is reliable even when run from inside a sub-worktree.
    let is_main_worktree = info.path.join(".git").is_dir();
    if is_main_worktree {
        return Err(AppError::CannotRemoveMainWorktree { path: info.path });
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
