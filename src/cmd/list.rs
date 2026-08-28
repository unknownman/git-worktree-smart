use crate::error::AppError;
use crate::git;
use crate::output;
use crate::Context;

pub fn run(ctx: &Context) -> Result<(), AppError> {
    let mut worktrees = git::get_worktrees(ctx.verbose)?;

    // Resolve the main repo root once and pre-canonicalize it so the sort
    // closure below never performs filesystem syscalls (avoids repeated
    // `canonicalize` per comparison).
    let repo_root = git::get_repo_root(ctx.verbose)
        .ok()
        .and_then(|root| std::fs::canonicalize(&root).ok());

    // Compute each worktree's sort key ONCE, upfront, using a single
    // canonicalize per worktree. The sort itself then runs purely in memory on
    // these precomputed keys — no filesystem syscalls inside the comparator.
    let mut indexed: Vec<(usize, bool)> = worktrees
        .iter()
        .enumerate()
        .map(|(idx, wt)| {
            let canon = std::fs::canonicalize(&wt.path).ok();
            let is_main = repo_root
                .as_ref()
                .is_some_and(|root| canon.as_deref() == Some(root));
            (idx, is_main)
        })
        .collect();

    indexed.sort_by(|(ia, a_main), (ib, b_main)| match (a_main, b_main) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => worktrees[*ia].name.cmp(&worktrees[*ib].name),
    });

    // Reassemble `worktrees` in the sorted order. Worktree counts are small, so
    // the per-element clone is negligible and keeps the logic obviously correct.
    let ordered: Vec<crate::models::WorktreeInfo> = indexed
        .iter()
        .map(|(idx, _)| worktrees[*idx].clone())
        .collect();
    worktrees = ordered;

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
