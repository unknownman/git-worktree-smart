use crate::error::AppError;
use crate::git;
use crate::git::command::{check_branch_exists, check_remote_branch_exists, get_repo_root};
use crate::git::ops;
use crate::git::parse::infer_worktree_path;
use crate::output;
use crate::Context;

pub fn run(
    ctx: &Context,
    name: &str,
    base: Option<&str>,
    track: Option<&str>,
) -> Result<(), AppError> {
    let repo_root = get_repo_root(ctx.verbose)?;
    let target_path = infer_worktree_path(&repo_root, name)?;

    if target_path.exists() {
        return Err(AppError::PathAlreadyExists { path: target_path });
    }

    let branch_exists = check_branch_exists(name, ctx.verbose)?;
    let remote_branch_exists = check_remote_branch_exists(name, ctx.verbose)?;

    if branch_exists && (base.is_some() || track.is_some()) {
        return Err(AppError::BranchAlreadyExistsCannotSpecifyBaseOrTrack {
            branch: name.to_owned(),
        });
    }

    // If the branch already exists AND is currently checked out in another
    // worktree, `git worktree add` would fatal with a raw stderr message.
    // Detect it up front and return a clean, actionable error instead.
    if branch_exists {
        let worktrees = git::get_worktrees(ctx.verbose)?;
        if let Some(existing) = worktrees
            .iter()
            .find(|wt| wt.branch.as_deref() == Some(name))
        {
            return Err(AppError::BranchAlreadyCheckedOut {
                branch: name.to_owned(),
                path: existing.path.clone(),
            });
        }
    }

    ops::add_worktree(
        ctx.verbose,
        &target_path,
        name,
        base,
        track,
        branch_exists,
        remote_branch_exists,
    )?;

    // Build the worktree's completion info from the known path + branch with a
    // single HEAD query — no need to re-scan every worktree in the repository.
    let info = git::get_worktree_info(&target_path, Some(name), ctx.verbose)?;

    if ctx.json {
        output::json::print_single(&info)?;
    } else {
        output::human::print_add_success(&info);
    }

    Ok(())
}
