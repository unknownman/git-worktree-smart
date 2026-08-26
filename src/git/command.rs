use std::path::Path;
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
        let message = if status.stderr.is_empty() {
            format!("git {} failed (exit code unknown)", args.join(" "))
        } else {
            status.stderr
        };
        return Err(AppError::GitError { message });
    }

    Ok(status.stdout)
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
