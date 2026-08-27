use crate::error::AppError;
use crate::git;
use crate::output;
use crate::Context;

pub fn run(ctx: &Context) -> Result<(), AppError> {
    let mut worktrees = git::get_worktrees(ctx.verbose)?;

    worktrees.sort_by(|a, b| {
        let a_is_main = a.path.join(".git").is_dir();
        let b_is_main = b.path.join(".git").is_dir();
        match (a_is_main, b_is_main) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    // Determine the active worktree from Git's perspective. This is more
    // reliable than comparing the cwd with `starts_with`, which breaks on
    // symlinked filesystems (e.g. macOS `/tmp` -> `/private/tmp`).
    let current_toplevel =
        crate::git::command::run_git(&["rev-parse", "--show-toplevel"], None, false).ok();
    let current_idx = current_toplevel.and_then(|toplevel| {
        let toplevel_path = std::path::Path::new(&toplevel);
        // Canonicalize both sides so symlink mismatches (e.g. /tmp vs
        // /private/tmp) don't defeat the active-worktree indicator.
        let toplevel_canon =
            std::fs::canonicalize(toplevel_path).unwrap_or_else(|_| toplevel_path.to_path_buf());
        worktrees.iter().position(|wt| {
            let wt_canon = std::fs::canonicalize(&wt.path).unwrap_or_else(|_| wt.path.clone());
            wt_canon == toplevel_canon
        })
    });

    if ctx.json {
        output::json::print_list(&worktrees)?;
    } else {
        output::human::print_list(&worktrees, current_idx.map(|i| worktrees[i].path.as_path()));
    }

    Ok(())
}
