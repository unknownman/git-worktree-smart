use crate::error::AppError;
use crate::git;
use crate::output;
use crate::Context;

pub fn run(ctx: &Context) -> Result<(), AppError> {
    let mut worktrees = git::get_worktrees(ctx.verbose)?;

    worktrees.sort_by(|a, b| a.name.cmp(&b.name));

    let current_dir = std::env::current_dir().ok();
    let current_path = current_dir.as_deref();

    let current_idx =
        current_path.and_then(|cwd| worktrees.iter().position(|wt| cwd.starts_with(&wt.path)));

    if ctx.json {
        output::json::print_list(&worktrees)?;
    } else {
        output::human::print_list(&worktrees, current_idx.map(|i| worktrees[i].path.as_path()));
    }

    Ok(())
}
