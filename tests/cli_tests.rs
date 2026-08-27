use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

fn wt() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("wt").unwrap()
}

/// Create a throwaway git repo and return its path, with a `Command` already
/// set to run inside it.
fn repo() -> (tempfile::TempDir, assert_cmd::Command) {
    let dir = tempfile::tempdir().expect("create tempdir");
    let status = Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(dir.path())
        .status()
        .expect("git init");
    assert!(status.success(), "git init failed");

    let mut cmd = wt();
    cmd.current_dir(dir.path());
    (dir, cmd)
}

#[test]
fn list_empty_repo_outputs_valid_json() {
    let (_dir, mut cmd) = repo();
    cmd.arg("--json")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("["))
        .stdout(predicate::str::contains("]"));
}

#[test]
fn default_command_is_list() {
    let (_dir, mut cmd) = repo();
    cmd.assert().success();
}

#[test]
fn list_short_alias_works() {
    let (_dir, mut cmd) = repo();
    cmd.arg("ls").assert().success();
}

#[test]
fn missing_required_argument_errors() {
    // `add` requires a positional `name` argument.
    wt().arg("add")
        .assert()
        .code(2) // clap error exit code for usage errors
        .stderr(predicate::str::contains("required"));
}

#[test]
fn unknown_subcommand_errors() {
    wt().arg("definitely-not-a-command")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn version_flag_works() {
    wt().arg("--version").assert().success();
}

#[test]
fn help_flag_works() {
    wt().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Git worktree manager"));
}

#[test]
fn not_a_git_repo_error_goes_to_stderr_and_json() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let mut cmd = wt();
    cmd.current_dir(dir.path())
        .arg("--json")
        .arg("list")
        .assert()
        .code(1)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("error"));
}

#[test]
fn add_creates_worktree_and_list_round_trips() {
    let (dir, mut cmd) = repo();
    let repo_root = dir.path().to_path_buf();

    // Create a first commit so a branch can be made.
    let commit = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(&repo_root)
        .status()
        .expect("git commit");
    assert!(commit.success());

    cmd.arg("add")
        .arg("feature/login")
        .assert()
        .success()
        .stdout(predicate::str::contains("feature/login"));

    // The worktree should now appear in the list.
    let mut list = wt();
    list.current_dir(&repo_root)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("feature/login"));

    // Clean up any linked worktrees via native git so the tempdir can be removed.
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(&repo_root)
        .status();
}

#[test]
fn prune_json_output_is_array() {
    let (_dir, mut cmd) = repo();
    cmd.arg("prune")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("["));
}
