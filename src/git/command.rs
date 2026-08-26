use std::path::Path;

use crate::error::AppError;

pub struct GitOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

pub fn run_git(dir: &Path, args: &[&str]) -> Result<GitOutput, AppError> {
    todo!("execute git in {dir:?} with args {args:?}")
}

pub fn run_git_quiet(_dir: &Path, _args: &[&str]) -> Result<String, AppError> {
    todo!()
}
