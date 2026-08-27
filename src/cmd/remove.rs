use crate::error::AppError;
use crate::git;
use crate::git::command::get_repo_root;
use crate::output;
use crate::Context;

pub fn run(ctx: &Context, target: &str, force: bool) -> Result<(), AppError> {
    let info = git::resolve_worktree(ctx, target)?;

    let repo_root = get_repo_root(ctx.verbose)?;
    if info.path == repo_root {
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
