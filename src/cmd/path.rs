use crate::error::AppError;
use crate::git;
use crate::Context;

pub fn run(ctx: &Context, target: &str) -> Result<(), AppError> {
    let info = git::resolve_worktree(ctx, target)?;
    println!("{}", info.path.display());
    Ok(())
}
