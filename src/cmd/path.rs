use crate::error::AppError;
use crate::git;
use crate::output;
use crate::Context;

pub fn run(ctx: &Context, target: &str) -> Result<(), AppError> {
    let info = git::resolve_worktree(ctx, target)?;

    if ctx.json {
        output::json::print_single(&info)?;
    } else {
        println!("{}", info.path.display());
    }

    Ok(())
}
