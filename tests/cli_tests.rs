use std::process::Command;

use predicates::prelude::*;

fn wt() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("wt").unwrap()
}

/// Create a throwaway git repo and return its path, with a `Command` already
/// set to run inside it.
///
/// Forces the initial branch to `main` so tests are independent of the user's
/// global `init.defaultBranch` setting (e.g. systems defaulting to `master`).
fn repo() -> (tempfile::TempDir, assert_cmd::Command) {
    let dir = tempfile::tempdir().expect("create tempdir");
    let status = Command::new("git")
        .args(["init", "-q", "-b", "main"])
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
fn add_in_empty_repo_returns_clean_error() {
    let (_dir, mut cmd) = repo();
    cmd.arg("add")
        .arg("feature/login")
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "Cannot create worktree in an empty repository",
        ));
}

#[test]
fn add_custom_path_creates_worktree_at_that_path() {
    let (dir, _cmd) = repo();
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    // Create the worktree at an explicit custom path (a sibling directory),
    // distinct from the automatically-inferred path.
    let custom = root.join("custom-worktree");

    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("add")
        .arg("feature/login")
        .arg("--path")
        .arg(&custom)
        .assert()
        .success()
        .stdout(predicate::str::contains("feature/login"));

    // The custom path must now exist and contain the checked-out branch.
    assert!(custom.is_dir(), "custom path was not created");
    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&custom)
        .output()
        .expect("git branch --show-current");
    assert!(branch.status.success());
    let branch = String::from_utf8(branch.stdout).expect("utf8");
    assert_eq!(branch.trim(), "feature/login");

    cleanup_worktree(&root, "feature/login");
    let _ = dir;
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
        .args(["init", "-q", "-b", "main"])
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
    let expected = dunce::canonicalize(&linked)
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
        .args(["init", "-q", "-b", "main"])
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
    let expected = dunce::canonicalize(&root).expect("canonicalize root");
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
    let login_path = dunce::canonicalize(root.join(sibling_name(&root, "feature-login")))
        .expect("canonicalize login path");
    let api_path =
        dunce::canonicalize(root.join(sibling_name(&root, "api"))).expect("canonicalize api path");

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

#[test]
fn test_remove_detached_worktree_with_unreachable_commits_fails_without_force() {
    let (dir, root, linked) = repo_with_worktree();

    // Detach HEAD in the linked worktree and add a commit reachable ONLY from
    // the detached HEAD (no branch points at it). Removing without --force must
    // refuse, or the commit would be orphaned and lost.
    let detach = Command::new("git")
        .args(["checkout", "--detach"])
        .current_dir(&linked)
        .status()
        .expect("git checkout --detach");
    assert!(detach.success());

    let commit = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "only on detached head"])
        .current_dir(&linked)
        .status()
        .expect("git commit");
    assert!(commit.success());

    // Resolve by absolute path (the branch name is no longer valid once
    // detached) and attempt removal from the main repo root.
    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("remove")
        .arg(linked.to_string_lossy().as_ref())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("detached HEAD state"));

    cleanup_worktree(&root, "feature/demo");
    let _ = dir;
}

#[test]
fn test_bare_repository_returns_clean_error() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let bare = dir.path().join("bare.git");
    let init = Command::new("git")
        .args(["init", "--bare", "-q"])
        .arg(&bare)
        .status()
        .expect("git init --bare");
    assert!(init.success());

    // `wt add` triggers `get_repo_root` (--show-toplevel), which fails in a
    // bare repo; it must surface a clean, actionable error instead of a raw
    // git fatal.
    let mut cmd = wt();
    cmd.current_dir(&bare)
        .arg("add")
        .arg("feature/x")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("bare repository"));
}

#[test]
fn test_add_existing_branch_with_base_or_track_is_rejected() {
    let (dir, root) = {
        let d = tempfile::tempdir().expect("create tempdir");
        let r = d.path().to_path_buf();
        init_repo_with_commit(&r);
        (d, r)
    };

    // Create a local branch (not checked out anywhere) so the inferred sibling
    // path does not already exist.
    let branch = Command::new("git")
        .args(["branch", "existing"])
        .current_dir(&root)
        .status()
        .expect("git branch existing");
    assert!(branch.success());

    // Specifying --base or --track for an already-existing branch must be
    // rejected cleanly (renamed BranchAlreadyExistsCannotSpecifyBaseOrTrack).
    // `base` is a positional argument, so pass it as the second value.
    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("add")
        .arg("existing")
        .arg("main")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("already exists locally"));

    let _ = dir;
}

#[test]
fn test_remove_branch_with_upstream_ahead_fails_without_force() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    // Set up a mock remote (bare repo) and establish an upstream for main.
    let bare = dir.path().join("remote.git");
    let init = Command::new("git")
        .args(["init", "--bare", "-q"])
        .arg(&bare)
        .status()
        .expect("git init --bare");
    assert!(init.success());

    let add_remote = Command::new("git")
        .args(["remote", "add", "origin"])
        .arg(&bare)
        .current_dir(&root)
        .status()
        .expect("git remote add origin");
    assert!(add_remote.success());

    let push_main = Command::new("git")
        .args(["push", "-q", "-u", "origin", "main"])
        .current_dir(&root)
        .status()
        .expect("git push origin main");
    assert!(push_main.success());

    // Create a linked worktree on a new branch.
    let mut add = wt();
    add.current_dir(&root)
        .arg("add")
        .arg("feature/ahead")
        .assert()
        .success();

    // Locate the linked worktree's path.
    let mut path = wt();
    let linked_out = path
        .current_dir(&root)
        .arg("path")
        .arg("feature/ahead")
        .output()
        .expect("wt path");
    let linked =
        std::path::PathBuf::from(String::from_utf8(linked_out.stdout).expect("utf8").trim());

    // Push the branch to establish an upstream (origin/feature/ahead).
    let push_branch = Command::new("git")
        .args(["push", "-q", "-u", "origin", "feature/ahead"])
        .current_dir(&linked)
        .status()
        .expect("git push feature/ahead");
    assert!(push_branch.success());

    // Commit locally so the branch is now 1 commit ahead of its upstream.
    let commit = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "ahead of upstream"])
        .current_dir(&linked)
        .status()
        .expect("git commit in linked worktree");
    assert!(commit.success());

    // Without --force: must refuse removal of an unpushed commit.
    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("remove")
        .arg("feature/ahead")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("unpushed commit"));

    // With --force: removal must succeed.
    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("remove")
        .arg("--force")
        .arg("feature/ahead")
        .assert()
        .success();

    cleanup_worktree(&root, "feature/ahead");
    let _ = dir;
}

#[test]
fn test_prune_execution_with_yes_flag() {
    let (_dir, root, linked) = repo_with_worktree();

    // Deleting the worktree directory makes the reference stale.
    std::fs::remove_dir_all(&linked).expect("remove worktree dir");

    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("prune")
        .arg("--yes")
        .assert()
        .success()
        // ANSI color codes wrap the message segments, so assert on the
        // contiguous, uncolored portion that confirms a single stale worktree
        // was pruned.
        .stdout(predicate::str::contains("stale worktree(s)"));

    // The directory is gone and its git reference was pruned; nothing left to
    // clean up outside the tempdir.
    let _ = root;
}

#[test]
fn test_fuzzy_match_threshold_rejects_unrelated_query() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    let mut add = wt();
    add.current_dir(&root)
        .arg("add")
        .arg("feature/login")
        .assert()
        .success();

    // "zzz" shares no meaningful part with any worktree name/branch; it must be
    // rejected as not found rather than falsely matching on weak characters.
    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("path")
        .arg("zzz")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("No worktree found"));

    cleanup_worktree(&root, "feature/login");
    let _ = dir;
}

#[test]
fn test_add_with_relative_path_canonicalizes_cleanly() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    // A relative path with `..` segments, resolved against the cwd. After the
    // worktree is created, the reported path must be the clean canonicalized
    // absolute path (i.e. ./custom/.. is normalized away).
    let mut cmd = wt();
    cmd.current_dir(&root)
        .arg("add")
        .arg("feature/rel")
        .arg("--path")
        .arg("./custom/../nested/worktree")
        .assert()
        .success();

    let nested = root.join("nested/worktree");
    assert!(nested.is_dir(), "worktree not created at nested path");

    // `wt add` (and `wt list`) should report the canonicalized absolute path.
    let expected = dunce::canonicalize(&nested).expect("canonicalize nested path");

    let mut list = wt();
    let out = list
        .current_dir(&root)
        .arg("list")
        .arg("--json")
        .output()
        .expect("wt list --json");
    assert!(out.status.success(), "wt list --json failed");
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert!(
        stdout.contains(&expected.to_string_lossy().into_owned()),
        "wt list --json did not report clean canonicalized path: {stdout}"
    );

    cleanup_worktree(&root, "feature/rel");
    let _ = dir;
}

#[test]
fn add_existing_local_branch_checks_it_out() {
    let (dir, _cmd) = repo();
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    // Create a branch locally WITHOUT checking it out anywhere. `wt add` must
    // check it out in the new worktree rather than trying to create a new one.
    let branch = Command::new("git")
        .args(["branch", "existing-feature"])
        .current_dir(&root)
        .status()
        .expect("git branch existing-feature");
    assert!(branch.success());

    let mut add = wt();
    add.current_dir(&root)
        .arg("add")
        .arg("existing-feature")
        .assert()
        .success()
        .stdout(predicate::str::contains("existing-feature"));

    // The inferred worktree directory must exist on disk.
    let mut path = wt();
    let linked = path
        .current_dir(&root)
        .arg("path")
        .arg("existing-feature")
        .output()
        .expect("wt path existing-feature");
    let linked = String::from_utf8(linked.stdout)
        .expect("utf8")
        .trim()
        .to_string();
    assert!(
        std::path::Path::new(&linked).is_dir(),
        "worktree directory not created: {linked}"
    );

    // The checked-out branch inside the new worktree must be the pre-existing
    // local branch (not a freshly-created one).
    let show = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&linked)
        .output()
        .expect("git branch --show-current");
    assert!(show.status.success());
    let branch = String::from_utf8(show.stdout).expect("utf8");
    assert_eq!(branch.trim(), "existing-feature");

    cleanup_worktree(&root, "existing-feature");
    let _ = dir;
}

#[test]
fn add_remote_only_branch_dwims_tracking() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    // Set up a bare remote and push `remote-feature` to it, then delete the
    // local branch so it exists ONLY on the remote.
    let bare = dir.path().join("remote.git");
    let init = Command::new("git")
        .args(["init", "--bare", "-q"])
        .arg(&bare)
        .status()
        .expect("git init --bare");
    assert!(init.success());

    let add_remote = Command::new("git")
        .args(["remote", "add", "origin"])
        .arg(&bare)
        .current_dir(&root)
        .status()
        .expect("git remote add origin");
    assert!(add_remote.success());

    let push_main = Command::new("git")
        .args(["push", "-q", "-u", "origin", "main"])
        .current_dir(&root)
        .status()
        .expect("git push origin main");
    assert!(push_main.success());

    let create = Command::new("git")
        .args(["branch", "remote-feature"])
        .current_dir(&root)
        .status()
        .expect("git branch remote-feature");
    assert!(create.success());

    let push_feature = Command::new("git")
        .args(["push", "-q", "-u", "origin", "remote-feature"])
        .current_dir(&root)
        .status()
        .expect("git push origin remote-feature");
    assert!(push_feature.success());

    let del = Command::new("git")
        .args(["branch", "-D", "remote-feature"])
        .current_dir(&root)
        .status()
        .expect("git branch -D remote-feature");
    assert!(del.success());

    // `wt add` must successfully DWIM-track the remote-only branch.
    let mut add = wt();
    add.current_dir(&root)
        .arg("add")
        .arg("remote-feature")
        .assert()
        .success()
        .stdout(predicate::str::contains("remote-feature"));

    let mut path = wt();
    let linked = path
        .current_dir(&root)
        .arg("path")
        .arg("remote-feature")
        .output()
        .expect("wt path remote-feature");
    let linked = String::from_utf8(linked.stdout)
        .expect("utf8")
        .trim()
        .to_string();
    assert!(
        std::path::Path::new(&linked).is_dir(),
        "worktree directory not created: {linked}"
    );

    // The new worktree's branch must have its upstream set via DWIM.
    let upstream = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "remote-feature@{u}"])
        .current_dir(&linked)
        .output()
        .expect("git rev-parse remote-feature@{u}");
    assert!(
        upstream.status.success(),
        "DWIM tracking not configured: {}",
        String::from_utf8_lossy(&upstream.stderr)
    );
    let upstream = String::from_utf8(upstream.stdout).expect("utf8");
    assert_eq!(upstream.trim(), "origin/remote-feature");

    cleanup_worktree(&root, "remote-feature");
    let _ = dir;
}

#[test]
fn add_with_explicit_track_creates_branch() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let root = dir.path().to_path_buf();
    init_repo_with_commit(&root);

    // Set up a bare remote and push a `base-feature` branch, then delete the
    // local "base-feature" so the only reference is on the remote.
    let bare = dir.path().join("remote.git");
    let init = Command::new("git")
        .args(["init", "--bare", "-q"])
        .arg(&bare)
        .status()
        .expect("git init --bare");
    assert!(init.success());

    let add_remote = Command::new("git")
        .args(["remote", "add", "origin"])
        .arg(&bare)
        .current_dir(&root)
        .status()
        .expect("git remote add origin");
    assert!(add_remote.success());

    let push_main = Command::new("git")
        .args(["push", "-q", "-u", "origin", "main"])
        .current_dir(&root)
        .status()
        .expect("git push origin main");
    assert!(push_main.success());

    let create = Command::new("git")
        .args(["branch", "base-feature"])
        .current_dir(&root)
        .status()
        .expect("git branch base-feature");
    assert!(create.success());

    let push_base = Command::new("git")
        .args(["push", "-q", "-u", "origin", "base-feature"])
        .current_dir(&root)
        .status()
        .expect("git push origin base-feature");
    assert!(push_base.success());

    let del = Command::new("git")
        .args(["branch", "-D", "base-feature"])
        .current_dir(&root)
        .status()
        .expect("git branch -D base-feature");
    assert!(del.success());

    // Create a NEW local branch `new-feature` that explicitly tracks
    // `origin/base-feature` as its upstream.
    let mut add = wt();
    add.current_dir(&root)
        .arg("add")
        .arg("new-feature")
        .arg("--track")
        .arg("origin/base-feature")
        .assert()
        .success()
        .stdout(predicate::str::contains("new-feature"));

    let mut path = wt();
    let linked = path
        .current_dir(&root)
        .arg("path")
        .arg("new-feature")
        .output()
        .expect("wt path new-feature");
    let linked = String::from_utf8(linked.stdout)
        .expect("utf8")
        .trim()
        .to_string();
    assert!(
        std::path::Path::new(&linked).is_dir(),
        "worktree directory not created: {linked}"
    );

    // The checked-out branch must be the newly-created `new-feature`.
    let show = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&linked)
        .output()
        .expect("git branch --show-current");
    assert!(show.status.success());
    let branch = String::from_utf8(show.stdout).expect("utf8");
    assert_eq!(branch.trim(), "new-feature");

    // Its upstream must be `origin/base-feature`.
    let upstream = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "new-feature@{u}"])
        .current_dir(&linked)
        .output()
        .expect("git rev-parse new-feature@{u}");
    assert!(
        upstream.status.success(),
        "upstream not configured: {}",
        String::from_utf8_lossy(&upstream.stderr)
    );
    let upstream = String::from_utf8(upstream.stdout).expect("utf8");
    assert_eq!(upstream.trim(), "origin/base-feature");

    cleanup_worktree(&root, "new-feature");
    let _ = dir;
}
