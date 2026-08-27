use std::process::Command;

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

/// Create a repo with an initial commit and one linked worktree on its own
/// branch. Returns the main repo root and the linked worktree's absolute path.
fn repo_with_worktree() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();

    let init = Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(&root)
        .status()
        .expect("git init");
    assert!(init.success());

    let commit = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(&root)
        .status()
        .expect("git commit");
    assert!(commit.success());

    // Add a linked worktree on its own branch (path inferred by `wt add`).
    let mut add = wt();
    add.current_dir(&root)
        .arg("add")
        .arg("feature/demo")
        .assert()
        .success();

    // Find the linked worktree path via `wt path`.
    let mut path = wt();
    let linked = path
        .current_dir(&root)
        .arg("path")
        .arg("feature/demo")
        .output()
        .expect("wt path");
    let linked = String::from_utf8(linked.stdout)
        .expect("utf8")
        .trim()
        .to_string();

    (dir, root, std::path::PathBuf::from(linked))
}

/// Remove a linked worktree via native git so no orphan directories are left
/// behind outside the tempdir (the linked worktree lives as a sibling of the
/// main repo's tempdir).
fn cleanup_worktree(root: &std::path::Path, name: &str) {
    let _ = Command::new("git")
        .args(["worktree", "remove", "--force", "--", name])
        .current_dir(root)
        .status();
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(root)
        .status();
}

#[test]
fn remove_main_repo_root_fails_with_error() {
    let (dir, root, _linked) = repo_with_worktree();

    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("remove")
        .arg("main")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Cannot remove the main worktree"))
        .stdout(predicate::str::is_empty());

    cleanup_worktree(&root, "feature/demo");
    let _ = dir;
}

#[test]
fn remove_dirty_worktree_fails_unless_force() {
    let (dir, root, linked) = repo_with_worktree();

    // Make the linked worktree dirty.
    std::fs::write(linked.join("untracked.txt"), "dirty").expect("write file");

    // Without --force: must fail with a dirty error.
    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("remove")
        .arg("feature/demo")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("uncommitted changes"));

    // With --force: must succeed.
    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("remove")
        .arg("--force")
        .arg("feature/demo")
        .assert()
        .success();

    cleanup_worktree(&root, "feature/demo");
    let _ = dir;
}

#[test]
fn remove_current_active_worktree_fails_even_with_force() {
    let (dir, root, linked) = repo_with_worktree();

    // Run `wt remove` FROM INSIDE the linked worktree: it must refuse to
    // remove the worktree we are currently standing in, even with --force.
    let mut cmd = wt();
    cmd.current_dir(&linked)
        .arg("remove")
        .arg("feature/demo")
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "Cannot remove the active worktree",
        ));

    let mut cmd = wt();
    cmd.current_dir(&linked)
        .arg("remove")
        .arg("--force")
        .arg("feature/demo")
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "Cannot remove the active worktree",
        ));

    cleanup_worktree(&root, "feature/demo");
    let _ = dir;
    let _ = root;
}

#[test]
fn path_prints_only_path_with_no_whitespace_or_ansi() {
    let (_dir, root, linked) = repo_with_worktree();

    let mut cmd = wt();
    let out = cmd
        .current_dir(&root)
        .arg("path")
        .arg("feature/demo")
        .output()
        .expect("wt path");
    assert!(out.status.success());

    let stdout = String::from_utf8(out.stdout).expect("utf8");
    // `canonicalize` resolves the temp-dir symlink on macOS, so compare against
    // the canonicalized path rather than the raw temp dir path.
    let expected = std::fs::canonicalize(&linked)
        .unwrap_or_else(|_| linked.clone())
        .to_string_lossy()
        .into_owned();
    let body = stdout.trim_end_matches(['\n', '\r']);
    // Output must contain nothing but the path: exactly one line, no ANSI
    // codes, no leading/trailing spaces or tabs.
    assert_eq!(body.lines().count(), 1, "more than one line: {stdout:?}");
    assert!(!body.contains(['\n', '\r']), "unexpected newlines in body");
    assert!(!body.contains('\x1b'), "ANSI escape codes present");
    assert_eq!(body, body.trim(), "leading/trailing spaces present");
    assert_eq!(body, expected, "path mismatch");

    cleanup_worktree(&root, "feature/demo");
}

#[test]
fn path_resolves_dot_from_inside_subdirectory() {
    let (dir, root, linked) = repo_with_worktree();

    // Create a subdirectory inside the linked worktree.
    let sub = linked.join("src").join("deep");
    std::fs::create_dir_all(&sub).expect("create subdir");

    // Run `wt path .` from inside that subdirectory; it must resolve to the
    // enclosing linked worktree.
    let mut cmd = wt();
    let out = cmd
        .current_dir(&sub)
        .arg("path")
        .arg(".")
        .output()
        .expect("wt path");
    assert!(out.status.success(), "wt path . failed");

    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.trim(),
        linked.to_string_lossy(),
        "did not resolve to worktree"
    );

    cleanup_worktree(&root, "feature/demo");
    let _ = dir;
    let _ = root;
}

/// Create a throwaway repo at `root` with an initial commit on the default
/// branch.
fn init_repo_with_commit(root: &std::path::Path) {
    let init = Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(root)
        .status()
        .expect("git init");
    assert!(init.success());

    let commit = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(root)
        .status()
        .expect("git commit");
    assert!(commit.success());
}

#[test]
fn test_subcommand_from_nested_subdirectory() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    let nested = root.join("src").join("nested").join("sub");
    std::fs::create_dir_all(&nested).expect("create nested dir");

    // `wt list` must not crash with an os error 2 (NotFound) from a cwd that
    // is deeper than the repo root.
    let mut list = wt();
    list.current_dir(&nested).arg("list").assert().success();

    // `wt path .` from the nested cwd must resolve to the enclosing worktree
    // (the main repo), not fail to canonicalize `.git`.
    let mut path = wt();
    let out = path
        .current_dir(&nested)
        .arg("path")
        .arg(".")
        .output()
        .expect("wt path .");
    assert!(out.status.success(), "wt path . failed: {out:?}");

    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let resolved = stdout.trim();
    let expected = std::fs::canonicalize(&root).expect("canonicalize root");
    assert_eq!(
        std::path::Path::new(resolved),
        expected.as_path(),
        "did not resolve nested cwd to repo root worktree"
    );
}

#[test]
fn test_add_already_checked_out_branch_fails_cleanly() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    // Check the branch out in the MAIN worktree so it already exists and is in
    // use, but its inferred sibling path (`../<repo>-feature/x`) does not yet
    // exist. This isolates the BranchAlreadyCheckedOut path from PathAlreadyExists.
    let co = Command::new("git")
        .args(["checkout", "-b", "feature/x"])
        .current_dir(&root)
        .status()
        .expect("git checkout -b");
    assert!(co.success());

    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("add")
        .arg("feature/x")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("already checked out"));
}

#[test]
fn test_case_insensitive_substring_and_short_query_resolution() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    for branch in ["feature/login", "api"] {
        let mut add = wt();
        add.current_dir(&root)
            .arg("add")
            .arg(branch)
            .assert()
            .success();
    }

    // Compute the inferred sibling paths and canonicalize for robustness on
    // symlinked file systems (e.g. /tmp -> /private/tmp on macOS).
    let login_path = std::fs::canonicalize(root.join(sibling_name(&root, "feature-login")))
        .expect("canonicalize login path");
    let api_path = std::fs::canonicalize(root.join(sibling_name(&root, "api")))
        .expect("canonicalize api path");

    // Uppercase substring query must match case-insensitively.
    let out = wt()
        .current_dir(&root)
        .arg("path")
        .arg("LOGIN")
        .output()
        .expect("wt path LOGIN");
    assert!(out.status.success(), "wt path LOGIN failed");
    let resolved = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        std::path::Path::new(resolved.trim()),
        login_path.as_path(),
        "uppercase LOGIN did not resolve to feature/login"
    );

    // A very short (single-word) query must still resolve, not false-negative.
    let out = wt()
        .current_dir(&root)
        .arg("path")
        .arg("api")
        .output()
        .expect("wt path api");
    assert!(out.status.success(), "wt path api failed");
    let resolved = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        std::path::Path::new(resolved.trim()),
        api_path.as_path(),
        "short query 'api' did not resolve"
    );

    cleanup_worktree(&root, "feature/login");
    cleanup_worktree(&root, "api");
    let _ = dir;
}

/// Compute the sibling worktree path `wt add` would infer for a branch, as a
/// sibling of `root` (i.e. `<parent>/<repo-name>-<sanitized>`).
fn sibling_name(root: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let parent = root.parent().expect("parent");
    let name = root.file_name().expect("name").to_string_lossy();
    parent.join(format!("{name}-{suffix}"))
}
