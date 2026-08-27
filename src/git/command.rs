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
        if status.stderr.contains("not a git repository") {
            return Err(AppError::NotAGitRepository);
        }
        let message = if status.stderr.is_empty() {
            format!("git {} failed (exit code unknown)", args.join(" "))
        } else {
            status.stderr
        };
        return Err(AppError::GitError { message });
    }

    Ok(status.stdout)
}

/// Like [`run_git`] but returns the captured `stderr` on success.
///
/// Some git subcommands (e.g. `worktree prune --dry-run --verbose`) write
/// their human-readable output to stderr even on success.
pub fn run_git_stderr(
    args: &[&str],
    cwd: Option<&Path>,
    verbose: bool,
) -> Result<String, AppError> {
    let status = run_git_status(args, cwd, verbose)?;

    if !status.success {
        if status.stderr.contains("not a git repository") {
            return Err(AppError::NotAGitRepository);
        }
        let message = if status.stderr.is_empty() {
            format!("git {} failed (exit code unknown)", args.join(" "))
        } else {
            status.stderr
        };
        return Err(AppError::GitError { message });
    }

    Ok(status.stderr)
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

    let output = cmd.output()?;

    Ok(CommandStatus {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

pub fn get_repo_root(verbose: bool) -> Result<PathBuf, AppError> {
    // `--git-common-dir` always points at the main repository's `.git`, even
    // when invoked from inside a linked worktree. We use it to derive the true
    // main repo root instead of `--show-toplevel`, which would return the
    // current worktree's root and cause incorrect sibling path inference.
    let output = run_git(&["rev-parse", "--git-common-dir"], None, verbose)?;

    let mut git_dir = PathBuf::from(output);

    // The common dir may be reported relative (e.g. `.git` from the main root);
    // convert to an absolute path before canonicalizing.
    if !git_dir.is_absolute() {
        let cwd = std::env::current_dir().map_err(AppError::Io)?;
        git_dir = cwd.join(git_dir);
    }

    let git_dir = std::fs::canonicalize(&git_dir).map_err(AppError::Io)?;

    // The main repository root is the parent of its `.git` directory.
    git_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::GitError {
            message: format!(
                "cannot determine repository root from `{}`",
                git_dir.display()
            ),
        })
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
