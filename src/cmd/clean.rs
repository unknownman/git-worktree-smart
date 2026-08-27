use crate::error::AppError;
use crate::git::ops;
use crate::git::parse::get_stale_worktrees;
use crate::output;
use crate::Context;

pub fn run(ctx: &Context, yes: bool) -> Result<(), AppError> {
    if yes {
        let stale = get_stale_worktrees(ctx.verbose)?;
        ops::prune_worktrees(ctx.verbose)?;

        if ctx.json {
            output::json::print_prune(&stale)?;
        } else {
            output::human::print_prune_success(&stale);
        }
        return Ok(());
    }

    let stale = get_stale_worktrees(ctx.verbose)?;

    if ctx.json {
        output::json::print_prune(&stale)?;
    } else {
        output::human::print_prune_dry_run(&stale);
    }

    Ok(())
}
