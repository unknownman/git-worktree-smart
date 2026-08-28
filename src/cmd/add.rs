use std::path::Path;

use crate::error::AppError;
use crate::git;
use crate::git::command::{
    check_branch_exists, check_remote_branch_exists, get_main_worktree_root,
};
use crate::git::ops;
use crate::git::parse::infer_worktree_path;
use crate::output;
use crate::Context;

pub fn run(
    ctx: &Context,
    name: &str,
    base: Option<&str>,
    track: Option<&str>,
    path: Option<&Path>,
) -> Result<(), AppError> {
    let main_root = get_main_worktree_root(ctx.verbose)?;

    // Linked worktrees cannot be added to a bare repository. Detect it up front
    // (mirroring the implicit check that `--show-toplevel` used to perform) so
    // we surface a clean, actionable error instead of a raw git fatal.
    let bare = crate::git::command::run_git_status(
        &["rev-parse", "--is-bare-repository"],
        None,
        ctx.verbose,
    )?
    .stdout
    .trim()
    .eq_ignore_ascii_case("true");
    if bare {
        return Err(AppError::BareRepositoryNotSupported);
    }

    // An empty repository (no commits) cannot back a linked worktree. Detect it
    // up front and return a clean, actionable error instead of a raw git fatal
    // ("invalid reference: HEAD").
    let has_commits = crate::git::command::run_git_status(
        &["rev-parse", "--verify", "--quiet", "HEAD"],
        None,
        ctx.verbose,
    )?
    .success;
    if !has_commits {
        return Err(AppError::EmptyRepository);
    }

    let target_path = match path {
        Some(p) => {
            // Normalize relative paths against the current working directory and
            // canonicalize to collapse `.`/`..` and symlinks.
            let abs = if p.is_absolute() {
                p.to_path_buf()
            } else {
                std::env::current_dir()?.join(p)
            };
            match dunce::canonicalize(&abs) {
                Ok(canon) => canon,
                Err(_) => abs,
            }
        }
        None => infer_worktree_path(&main_root, name)?,
    };

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

    // Native `git worktree add` does not create missing intermediate
    // directories, so pre-create the parent of the target path when the user
    // supplies a custom location (e.g. `--path ../missing/dir`). `create_dir_all`
    // is a no-op if the directories already exist.
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
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

    // `target_path` could not be canonicalized before the directory existed.
    // Re-canonicalize now that the worktree exists so the reported path matches
    // `wt list` (e.g. macOS resolves `/tmp` -> `/private/tmp`).
    let final_path = dunce::canonicalize(&target_path).unwrap_or_else(|_| target_path.clone());

    // Build the worktree's completion info from the known path + branch with a
    // single HEAD query — no need to re-scan every worktree in the repository.
    let info = git::get_worktree_info(&final_path, Some(name), ctx.verbose)?;

    if ctx.json {
        output::json::print_single(&info)?;
    } else {
        output::human::print_add_success(&info);
    }

    Ok(())
}
