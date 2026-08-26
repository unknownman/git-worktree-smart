use std::path::Path;
use std::process::Command;

use crate::error::AppError;

pub fn run_git(args: &[&str], cwd: Option<&Path>, verbose: bool) -> Result<String, AppError> {
    if verbose {
        eprintln!("[EXEC] git {}", args.join(" "));
    }

    let mut cmd = Command::new("git");
    cmd.args(args);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = cmd.output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            format!(
                "git {} failed (exit code {})",
                args.join(" "),
                output.status
            )
        } else {
            stderr
        };
        return Err(AppError::GitError { message });
    }

    Ok(stdout)
}
