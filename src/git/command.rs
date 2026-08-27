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
        eprintln!("[EXEC] git {}", args.join(" "));
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

pub fn get_repo_root(verbose: bool) -> Result<PathBuf, AppError> {
    // First find the top-level directory of the current worktree. `--show-toplevel`
    // always returns an absolute path, so it is a stable anchor for resolving the
    // (possibly relative) common dir below.
    let toplevel = run_git(&["rev-parse", "--show-toplevel"], None, verbose)?;
    let toplevel = PathBuf::from(toplevel);

    // `--git-common-dir` always points at the main repository's `.git`, even
    // when invoked from inside a linked worktree. We use it to derive the true
    // main repo root instead of `--show-toplevel`, which would return the
    // current worktree's root and cause incorrect sibling path inference.
    //
    // Run from the worktree's toplevel so any relative result (`.git` or
    // `../.git`) resolves against that directory, NOT the process cwd — which
    // may be a nested subdirectory where `cwd.join(".git")` does not exist.
    let output = run_git(&["rev-parse", "--git-common-dir"], Some(&toplevel), verbose)?;

    let mut git_dir = PathBuf::from(output);

    // The common dir may be reported relative (e.g. `.git` from the main root);
    // resolve it relative to the worktree toplevel before canonicalizing.
    if !git_dir.is_absolute() {
        git_dir = toplevel.join(git_dir);
    }

    let git_dir = std::fs::canonicalize(&git_dir).map_err(AppError::Io)?;

    // When invoked from the main worktree, `git_dir` is the main `.git`
    // directory, whose parent is the main repository root. When invoked from a
    // linked worktree, `git_dir` points at the main `.git` (the common dir),
    // whose parent is again the main repository root.
    let root = git_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::GitError {
            message: format!(
                "cannot determine repository root from `{}`",
                git_dir.display()
            ),
        })?;

    // Canonicalize the derived root so it matches the canonicalized forms used
    // elsewhere (e.g. `resolve_from_worktrees` and `wt remove`).
    std::fs::canonicalize(&root).map_err(AppError::Io)
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

/// Returns `true` if the given commit is reachable from at least one branch
/// (i.e. is merged into any branch head).
///
/// This is used to detect detached-HEAD worktrees whose commits would become
/// orphaned (unreachable) and lost if the worktree is deleted without `--force`.
///
/// Uses `git for-each-ref --contains <hash> refs/heads` (rather than
/// `git branch --contains`, which also emits a detached-HEAD marker line even
/// when no real branch contains the commit) so only actual branch refs count.
pub fn is_commit_merged_or_reachable(
    hash: &str,
    cwd: &Path,
    verbose: bool,
) -> Result<bool, AppError> {
    if hash.is_empty() || hash.chars().all(|c| c == '0') {
        // No commits yet: nothing can be orphaned.
        return Ok(true);
    }
    // List every branch ref that contains the commit; non-empty means it is
    // reachable from at least one branch.
    let output = run_git(
        &["for-each-ref", "--contains", hash, "refs/heads"],
        Some(cwd),
        verbose,
    )?;
    Ok(!output.is_empty())
}
