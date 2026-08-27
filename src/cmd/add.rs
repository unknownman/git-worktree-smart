use crate::error::AppError;
use crate::git;
use crate::git::command::{check_branch_exists, get_repo_root};
use crate::git::ops;
use crate::git::parse::infer_worktree_path;
use crate::models::WorktreeInfo;
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
        return Err(AppError::WorktreeAlreadyExists { path: target_path });
    }

    let branch_exists = check_branch_exists(name, ctx.verbose)?;

    ops::add_worktree(ctx.verbose, &target_path, name, base, track, branch_exists)?;

    let worktrees = git::get_worktrees(ctx.verbose)?;
    let info = worktrees
        .into_iter()
        .find(|wt| wt.path == target_path)
        .unwrap_or_else(|| WorktreeInfo {
            path: target_path.clone(),
            name: name.to_owned(),
            branch: Some(name.to_owned()),
            head_hash: String::new(),
            head_msg: String::new(),
            status: crate::models::WorktreeStatus::clean(),
        });

    if ctx.json {
        output::json::print_single(&info)?;
    } else {
        output::human::print_add_success(&info);
    }

    Ok(())
}
