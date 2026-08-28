use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::AppError;

pub struct CommandStatus {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_git(args: &[&str], cwd: Option<&Path>, verbose: bool) -> Result<String, AppError> {
    let status = run_git_status(args, cwd, verbose)?;

    if !status.success {
        return Err(map_git_failure(args, &status));
    }

    Ok(status.stdout)
}

/// Like [`run_git`] but returns the captured combined output on success.
///
/// Some git subcommands (e.g. `worktree prune --dry-run --verbose`) write
/// their human-readable output to stderr, while others write to stdout. To
/// stay compatible across Git versions, combine both streams so no content is
/// ever missed.
pub fn run_git_stderr(
    args: &[&str],
    cwd: Option<&Path>,
    verbose: bool,
) -> Result<String, AppError> {
    let status = run_git_status(args, cwd, verbose)?;

    if !status.success {
        return Err(map_git_failure(args, &status));
    }

    Ok(combine_output(&status.stdout, &status.stderr))
}

/// Translate a failed `git` invocation into the most specific [`AppError`].
fn map_git_failure(args: &[&str], status: &CommandStatus) -> AppError {
    let stderr = &status.stderr;

    if stderr.contains("not a git repository") {
        return AppError::NotAGitRepository;
    }
    if stderr.contains("this operation must be run in a work tree")
        || stderr.contains("must be run in a work tree")
    {
        return AppError::BareRepositoryNotSupported;
    }
    let message = if stderr.is_empty() {
        format!("git {} failed (exit code unknown)", args.join(" "))
    } else {
        stderr.clone()
    };
    AppError::GitError { message }
}

fn combine_output(stdout: &str, stderr: &str) -> String {
    let mut combined = String::new();
    if !stdout.is_empty() {
        combined.push_str(stdout.trim());
        if !stderr.is_empty() {
            combined.push('\n');
        }
    }
    if !stderr.is_empty() {
        combined.push_str(stderr.trim());
    }
    combined
}

pub fn run_git_status(
    args: &[&str],
    cwd: Option<&Path>,
    verbose: bool,
) -> Result<CommandStatus, AppError> {
    if verbose {
        // `get_worktrees` runs worker threads in parallel; each prints a
        // `[EXEC]` line to stderr. Locking stderr keeps the write atomic so
        // concurrent lines never interleave and garble the console output.
        let stderr = std::io::stderr();
        let mut handle = stderr.lock();
        use std::io::Write;
        let _ = writeln!(handle, "[EXEC] git {}", args.join(" "));
    }

    let mut cmd = Command::new("git");
    cmd.args(args);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = match cmd.output() {
        Ok(o) => o,
        // The `git` executable itself could not be spawned (not on PATH).
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::GitNotFound);
        }
        Err(e) => return Err(AppError::Io(e)),
    };

    Ok(CommandStatus {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

/// Return the canonicalized root of the repository's **main** worktree.
///
/// `git worktree list --porcelain` always lists the main worktree first, so we
/// parse its path from the first `worktree <path>` entry — this avoids deriving
/// the root from `--git-common-dir`, which breaks for submodules and
/// `core.worktree` setups (the common dir lives inside the parent repository's
/// `.git/modules`).
///
/// However, Git reports a submodule's main worktree as its **gitdir** rather
/// than the checked-out directory (because the worktree's `.git` is a file
/// pointing elsewhere, so Git cannot strip `.git` from the path). Detect that
/// case — when the reported path is the git common directory itself — and fall
/// back to the real worktree root from `--show-toplevel`.
pub fn get_main_worktree_root(verbose: bool) -> Result<PathBuf, AppError> {
    let output = run_git(&["worktree", "list", "--porcelain"], None, verbose)?;

    let reported = output
        .lines()
        .find_map(|line| line.strip_prefix("worktree ").map(PathBuf::from))
        .ok_or_else(|| AppError::GitError {
            message: "cannot determine the main worktree root".to_owned(),
        })?;

    let common_dir = get_git_common_dir(verbose)?;

    // If the reported main-worktree path is actually the git common directory
    // (a submodule's gitdir), it is not a real worktree — fall back to the true
    // worktree root reported by `--show-toplevel`.
    if path_is_within(&reported, &common_dir) {
        return get_current_worktree_root(verbose);
    }

    dunce::canonicalize(&reported).map_err(AppError::Io)
}

/// Return the canonicalized root of the *current* worktree (`--show-toplevel`).
fn get_current_worktree_root(verbose: bool) -> Result<PathBuf, AppError> {
    let toplevel = run_git(&["rev-parse", "--show-toplevel"], None, verbose)?;
    dunce::canonicalize(&toplevel).map_err(AppError::Io)
}

/// Returns `true` if `path` equals or is a descendant of `ancestor`.
fn path_is_within(path: &Path, ancestor: &Path) -> bool {
    // Both arguments are absolute and already canonicalized by callers, so a
    // plain component-wise prefix comparison is sufficient.
    let path = path.to_string_lossy();
    let ancestor = ancestor.to_string_lossy();
    if ancestor.is_empty() {
        return false;
    }
    path == ancestor || path.starts_with(&format!("{ancestor}/"))
}

/// Return the canonicalized path of the repository's Git **common directory**.
///
/// This is where worktree admin directories live (`<common>/worktrees/<name>`),
/// regardless of whether the repo is a submodule or uses `core.worktree`. The
/// result may be reported relative by Git, so it is resolved against the current
/// working directory before canonicalizing.
pub fn get_git_common_dir(verbose: bool) -> Result<PathBuf, AppError> {
    let output = run_git(&["rev-parse", "--git-common-dir"], None, verbose)?;
    let mut path = PathBuf::from(output);

    if !path.is_absolute() {
        let cwd = std::env::current_dir().map_err(AppError::Io)?;
        path = cwd.join(path);
    }

    dunce::canonicalize(&path).map_err(AppError::Io)
}

pub fn check_branch_exists(branch_name: &str, verbose: bool) -> Result<bool, AppError> {
    let status = run_git_status(
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch_name}"),
        ],
        None,
        verbose,
    )?;
    Ok(status.success)
}

/// Returns `true` if a branch of the given name exists on any configured
/// remote (e.g. `origin/branch_name`).
///
/// Uses `for-each-ref` rather than `git branch --list "*/<branch>"` because the
/// `*` glob does not match slashes (`/`), so it would never match slashed
/// branches like `feature/login` on `origin/feature/login`.
pub fn check_remote_branch_exists(branch_name: &str, verbose: bool) -> Result<bool, AppError> {
    let output = run_git(
        &[
            "for-each-ref",
            "--format=%(refname:strip=2)",
            "refs/remotes",
        ],
        None,
        verbose,
    )?;

    // Each line is `<remote>/<branch...>` (e.g. `origin/feature/login`);
    // compare the branch portion (after the first `/`) to the looked-up name.
    Ok(output.lines().any(|line| {
        line.split_once('/')
            .map(|(_, branch)| branch == branch_name)
            .unwrap_or(false)
    }))
}

/// Returns `true` if the given commit is reachable from at least one local or
/// remote branch (i.e. is merged into any branch head, or pushed to a remote
/// tracking branch).
///
/// This is used to detect detached-HEAD worktrees whose commits would become
/// orphaned (unreachable) and lost if the worktree is deleted without `--force`.
///
/// Uses `git for-each-ref --contains <hash>` across both `refs/heads` (local)
/// and `refs/remotes` (remote tracking) so that commits pushed to a remote but
/// not yet merged into a local branch are still considered reachable.
pub fn is_commit_merged_or_reachable(
    hash: &str,
    cwd: &Path,
    verbose: bool,
) -> Result<bool, AppError> {
    if hash.is_empty() || hash.chars().all(|c| c == '0') {
        // No commits yet: nothing can be orphaned.
        return Ok(true);
    }
    // List every local or remote branch ref that contains the commit;
    // non-empty means it is reachable from at least one branch.
    let output = run_git(
        &[
            "for-each-ref",
            "--contains",
            hash,
            "refs/heads",
            "refs/remotes",
        ],
        Some(cwd),
        verbose,
    )?;
    Ok(!output.is_empty())
}

#[cfg(test)]
mod tests {
    use super::path_is_within;
    use std::path::Path;

    #[test]
    fn path_equal_to_ancestor_is_within() {
        assert!(path_is_within(
            Path::new("/repo/super/.git/modules/sub"),
            Path::new("/repo/super/.git/modules/sub"),
        ));
    }

    #[test]
    fn path_descendant_of_ancestor_is_within() {
        assert!(path_is_within(
            Path::new("/repo/super/.git/modules/sub/worktrees/x"),
            Path::new("/repo/super/.git/modules/sub"),
        ));
    }

    #[test]
    fn ancestor_is_descendant_of_path_is_not_within() {
        assert!(!path_is_within(
            Path::new("/repo/super"),
            Path::new("/repo/super/.git"),
        ));
    }

    #[test]
    fn sibling_path_is_not_within() {
        assert!(!path_is_within(
            Path::new("/repo/super/sub"),
            Path::new("/repo/super/.git/modules"),
        ));
    }

    #[test]
    fn similar_prefix_but_not_within() {
        assert!(!path_is_within(
            Path::new("/repo/super/.git/modules/submarine"),
            Path::new("/repo/super/.git/modules/sub"),
        ));
    }
}
