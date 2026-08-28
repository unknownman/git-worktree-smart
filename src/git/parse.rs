use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::command::{get_repo_root, run_git, run_git_status, run_git_stderr};
use crate::models::{WorktreeInfo, WorktreeStatus};

pub fn get_worktrees(verbose: bool) -> Result<Vec<WorktreeInfo>, AppError> {
    let output = run_git(&["worktree", "list", "--porcelain"], None, verbose)?;
    let mut worktrees = parse_worktree_list(&output);

    // Fetch the head message and status for every worktree concurrently. Each
    // spawns several git subprocesses, so parallelizing avoids a long serial
    // stall. Failures for an individual worktree (corrupted index, permission
    // errors, missing dir) are caught and surfaced as stale/unreadable rather
    // than crashing the whole `wt list` — healthy worktrees still display.
    std::thread::scope(|scope| {
        let handles: Vec<_> = worktrees
            .iter_mut()
            .map(|wt| {
                scope.spawn(move || {
                    let head_ok = get_head_message(&wt.head_hash, &wt.path, verbose);
                    let status_ok = get_worktree_status(&wt.path, verbose);

                    match (head_ok, status_ok) {
                        (Ok(msg), Ok(status)) => {
                            wt.head_msg = msg;
                            wt.status = status;
                        }
                        (_, Ok(status)) => {
                            // Head message unreadable but status was read.
                            wt.head_msg = "(unreadable)".to_owned();
                            wt.status = status;
                        }
                        (_, _) => {
                            // Mark as unreadable so the UI can flag it instead
                            // of silently reporting a "clean" worktree.
                            wt.head_msg = "(unreadable)".to_owned();
                            wt.status.is_stale = true;
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }
    });

    Ok(worktrees)
}

pub fn parse_worktree_list(raw: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current: HashMap<String, String> = HashMap::new();

    for line in raw.lines() {
        if line.is_empty() {
            if let Some(wt) = flush_block(&current) {
                worktrees.push(wt);
            }
            current.clear();
        } else if let Some((key, value)) = line.split_once(' ') {
            current.insert(key.to_owned(), value.to_owned());
        } else {
            current.insert(line.to_owned(), String::new());
        }
    }

    if let Some(wt) = flush_block(&current) {
        worktrees.push(wt);
    }

    worktrees
}

fn flush_block(block: &HashMap<String, String>) -> Option<WorktreeInfo> {
    let raw_path = block.get("worktree")?;
    let path: PathBuf = raw_path.into();

    let head_hash = block.get("HEAD").cloned().unwrap_or_default();

    let branch = block
        .get("branch")
        .map(|b| b.strip_prefix("refs/heads/").unwrap_or(b).to_owned());

    let name = derive_name(&path, branch.as_deref());

    Some(WorktreeInfo {
        path,
        name,
        branch,
        head_hash,
        head_msg: String::new(),
        status: WorktreeStatus::clean(),
    })
}

fn derive_name(path: &Path, branch: Option<&str>) -> String {
    if let Some(b) = branch {
        return b.to_owned();
    }

    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_owned()
}

fn get_head_message(hash: &str, cwd: &Path, verbose: bool) -> Result<String, AppError> {
    // If the worktree directory no longer exists, `Command::current_dir` below
    // would fail with an OS I/O error. Surface it as stale instead.
    if !cwd.is_dir() {
        return Ok("(stale)".to_owned());
    }

    // The null hash means the repository has no commits yet (fresh `git init`).
    if hash.is_empty() || hash.chars().all(|c| c == '0') {
        return Ok("(no commits yet)".to_owned());
    }
    // Look the commit up within the worktree's own repository context.
    let raw = run_git(&["log", "-1", "--format=%s", hash], Some(cwd), verbose)?;
    // Sanitize whitespace so commit messages can never break table formatting.
    let sanitized: String = raw
        .chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            other => other,
        })
        .collect();
    Ok(sanitized.trim().to_owned())
}

pub fn get_worktree_status(cwd: &Path, verbose: bool) -> Result<WorktreeStatus, AppError> {
    // If the worktree's directory no longer exists (stale), there is nothing
    // to inspect — surface it as stale so the UI can flag the broken state
    // instead of pretending everything is clean.
    if !cwd.is_dir() {
        return Ok(WorktreeStatus {
            is_dirty: false,
            is_stale: true,
            ahead: 0,
            behind: 0,
        });
    }

    let is_dirty = check_dirty(cwd, verbose)?;
    let (ahead, behind) = get_ahead_behind(cwd, verbose)?;

    Ok(WorktreeStatus {
        is_dirty,
        is_stale: false,
        ahead,
        behind,
    })
}

/// Build a complete [`WorktreeInfo`] for a single known worktree without
/// scanning the whole repository. Used by `wt add` to report the freshly
/// created worktree efficiently instead of re-running the full parallel
/// `get_worktrees` sweep.
pub fn get_worktree_info(
    path: &Path,
    branch: Option<&str>,
    verbose: bool,
) -> Result<WorktreeInfo, AppError> {
    let name = derive_name(path, branch);

    let head_hash = if !path.is_dir() {
        String::new()
    } else {
        // Fetch only this worktree's HEAD so we don't rescan every worktree.
        run_git(
            &["rev-parse", "--verify", "--quiet", "HEAD"],
            Some(path),
            verbose,
        )
        .unwrap_or_default()
    };

    let head_msg = get_head_message(&head_hash, path, verbose)?;

    Ok(WorktreeInfo {
        path: path.to_path_buf(),
        name,
        branch: branch.map(|b| b.to_owned()),
        head_hash,
        head_msg,
        status: WorktreeStatus::clean(),
    })
}

pub fn parse_dirty(porcelain: &str) -> bool {
    !porcelain.trim().is_empty()
}

pub fn parse_rev_list_count(output: &str) -> (u32, u32) {
    let parts: Vec<&str> = output.split_whitespace().collect();
    if parts.len() < 2 {
        return (0, 0);
    }

    let ahead = parts[0].parse::<u32>().unwrap_or(0);
    let behind = parts[1].parse::<u32>().unwrap_or(0);

    (ahead, behind)
}

fn check_dirty(cwd: &Path, verbose: bool) -> Result<bool, AppError> {
    // `-uall` also reports untracked files inside nested untracked directories,
    // so we never miss uncommitted content.
    let output = run_git(&["status", "--porcelain", "-uall"], Some(cwd), verbose)?;
    Ok(parse_dirty(&output))
}

fn get_ahead_behind(cwd: &Path, verbose: bool) -> Result<(u32, u32), AppError> {
    // A detached HEAD has no branch to compare against, and its uncommitted-to-a-
    // branch commits are safety-checked separately by the caller (the detached-
    // HEAD reachability guard in `wt remove`). Reporting an "ahead" count here
    // would shadow that more specific safety path, so treat it as having no
    // upstream status.
    let symbolic_ref = run_git_status(
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        Some(cwd),
        verbose,
    );
    if !symbolic_ref.map(|s| s.success).unwrap_or(false) {
        return Ok((0, 0));
    }

    let upstream_check = run_git(
        &["rev-parse", "--verify", "--quiet", "@{u}"],
        Some(cwd),
        verbose,
    );

    if upstream_check.is_ok() {
        let output = run_git(
            &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
            Some(cwd),
            verbose,
        )?;
        return Ok(parse_rev_list_count(&output));
    }

    // No upstream tracking is configured. Fall back to comparing HEAD against a
    // likely default/base branch so local commits are still surfaced instead of
    // silently reporting the worktree as "clean". Candidates are tried in order:
    // the remote HEAD pointer, then common local and remote default branches.
    let candidates = [
        "refs/remotes/origin/HEAD",
        "refs/heads/main",
        "refs/heads/master",
        "refs/remotes/origin/main",
        "refs/remotes/origin/master",
    ];

    for base in candidates {
        // Resolve the base ref; skip it if it does not exist in this repo.
        let exists = run_git_status(
            &["rev-parse", "--verify", "--quiet", base],
            Some(cwd),
            verbose,
        );
        if !exists.map(|s| s.success).unwrap_or(false) {
            continue;
        }

        // Count commits reachable from HEAD but not from the base branch.
        let Ok(output) = run_git(
            &["rev-list", "--count", "HEAD", &format!("^{base}")],
            Some(cwd),
            verbose,
        ) else {
            continue;
        };
        let ahead = output.trim().parse::<u32>().unwrap_or(0);
        return Ok((ahead, 0));
    }

    // No upstream and no recognizable default branch. Treat as having nothing
    // ahead/behind rather than failing the whole command.
    Ok((0, 0))
}

pub fn sanitize_branch_name(branch: &str) -> String {
    branch
        .chars()
        .map(|c| if is_safe_path_char(c) { c } else { '-' })
        .collect()
}

fn is_safe_path_char(c: char) -> bool {
    c.is_alphanumeric() || c == '.' || c == '_' || c == '-'
}

pub fn infer_worktree_path(repo_root: &Path, branch_name: &str) -> Result<PathBuf, AppError> {
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::PathInferenceFailed {
            reason: format!("cannot extract repo name from {}", repo_root.display()),
        })?;

    let sanitized = sanitize_branch_name(branch_name);
    if sanitized.chars().all(|c| c == '-') {
        return Err(AppError::PathInferenceFailed {
            reason: "invalid branch name for worktree path".to_string(),
        });
    }
    let dir_name = format!("{repo_name}-{sanitized}");

    let parent = repo_root
        .parent()
        .ok_or_else(|| AppError::PathInferenceFailed {
            reason: format!("cannot determine parent of {}", repo_root.display()),
        })?;

    Ok(parent.join(dir_name))
}

/// Parse the output of `git worktree prune --dry-run --verbose`.
///
/// Git emits one line per stale worktree reference. We support both observed
/// formats:
///   * an absolute worktree path (older git, "Removing worktree: /a/b")
///   * a relative admin-dir entry (modern git, "Removing worktrees/<name>:
///     <reason>", where the identifier is relative to the git dir)
///
/// Returns the raw identifiers. Callers may resolve relative admin entries to
/// absolute worktree paths via [`resolve_stale_path`].
pub fn parse_prune_dry_run(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();

            // Modern git: "Removing worktrees/<name>: <reason>" — keep the
            // full relative identifier so it can be resolved later.
            if let Some(rest) = trimmed.strip_prefix("Removing worktrees/") {
                let name = rest.split(':').next().unwrap_or(rest).trim();
                return (!name.is_empty()).then(|| format!("worktrees/{name}"));
            }

            // Older git: "Removing worktree: /abs/path" — some versions wrap
            // paths containing spaces in quotes.
            if let Some(rest) = trimmed.strip_prefix("Removing worktree: ") {
                let path = strip_quotes(rest.split(':').next().unwrap_or(rest).trim());
                return (!path.is_empty()).then(|| path.to_owned());
            }

            None
        })
        .collect()
}

/// Strip matching leading/trailing single or double quotes from a path.
fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    let bytes = s.as_bytes();
    if s.len() >= 2
        && ((bytes[0] == b'"' && bytes[s.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[s.len() - 1] == b'\''))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Resolve a raw stale identifier from [`parse_prune_dry_run`] into an
/// absolute worktree path.
///
/// Handles both absolute paths (returned as-is) and relative admin-dir entries
/// of the form `worktrees/<name>`, resolved by reading the `gitdir` file that
/// points back to the (now missing) worktree.
pub fn resolve_stale_path(path: &str, repo_root: &Path) -> Result<PathBuf, AppError> {
    let p = Path::new(path);

    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }

    // Relative form: worktrees/<name>. The admin dir is <repo>/.git/worktrees/<name>.
    // Its `gitdir` file holds the path to the linked worktree's `.git` file;
    // the parent of that is the worktree root.
    let admin_dir = repo_root.join(".git").join(p);
    let gitdir_file = admin_dir.join("gitdir");

    // If the `gitdir` file cannot be read, gracefully fall back to the admin
    // dir itself (or the repo-root-joined path) rather than failing the whole
    // `prune` command. The fallback path is only used for display purposes.
    let Ok(contents) = std::fs::read_to_string(&gitdir_file) else {
        let fallback = if admin_dir.exists() {
            admin_dir
        } else {
            repo_root.join(path)
        };
        return Ok(fallback);
    };

    let git_file = PathBuf::from(contents.trim());
    let worktree_root = git_file
        .parent()
        .ok_or_else(|| AppError::GitError {
            message: format!(
                "cannot determine worktree root from `{}`",
                git_file.display()
            ),
        })?
        .to_path_buf();

    Ok(worktree_root)
}

/// Runs `git worktree prune --dry-run --verbose` and parses the stale paths,
/// resolving relative admin-dir entries to absolute worktree paths.
pub fn get_stale_worktrees(verbose: bool) -> Result<Vec<String>, AppError> {
    // `git worktree prune --dry-run --verbose` writes its listing to stderr.
    let output = run_git_stderr(
        &["worktree", "prune", "--dry-run", "--verbose"],
        None,
        verbose,
    )?;
    let raw = parse_prune_dry_run(&output);

    let repo_root = get_repo_root(verbose)?;

    raw.iter()
        .map(|entry| {
            resolve_stale_path(entry, &repo_root).map(|p| p.to_string_lossy().into_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGLE_WORKTREE: &str = "\
worktree /Users/dev/my-repo
HEAD abc1234
branch refs/heads/main";

    const DETACHED_HEAD: &str = "\
worktree /Users/dev/my-repo
HEAD deadbeef";

    const TWO_WORKTREES: &str = "\
worktree /Users/dev/my-repo
HEAD abc1234
branch refs/heads/main

worktree /Users/dev/my-repo-feature
HEAD def5678
branch refs/heads/feature/auth";

    const THREE_WORKTREES_DETACHED: &str = "\
worktree /Users/dev/my-repo
HEAD abc1234
branch refs/heads/main

worktree /Users/dev/my-repo-experiment
HEAD 1111111
branch refs/heads/experiment

worktree /tmp/worktrees/hotfix
HEAD 2222222";

    const WORKTREE_EXTRA_KEYS: &str = "\
worktree /Users/dev/my-repo
HEAD abc1234
branch refs/heads/main
bare
locked";

    #[test]
    fn test_parse_single_worktree() {
        let result = parse_worktree_list(SINGLE_WORKTREE);
        assert_eq!(result.len(), 1);

        let wt = &result[0];
        assert_eq!(wt.path, PathBuf::from("/Users/dev/my-repo"));
        assert_eq!(wt.name, "main");
        assert_eq!(wt.branch.as_deref(), Some("main"));
        assert_eq!(wt.head_hash, "abc1234");
        assert!(wt.head_msg.is_empty());
        assert!(wt.status.is_clean());
    }

    #[test]
    fn test_parse_detached_head() {
        let result = parse_worktree_list(DETACHED_HEAD);
        assert_eq!(result.len(), 1);

        let wt = &result[0];
        assert_eq!(wt.path, PathBuf::from("/Users/dev/my-repo"));
        assert_eq!(wt.name, "my-repo");
        assert!(wt.branch.is_none());
        assert_eq!(wt.head_hash, "deadbeef");
    }

    #[test]
    fn test_parse_two_worktrees() {
        let result = parse_worktree_list(TWO_WORKTREES);
        assert_eq!(result.len(), 2);

        assert_eq!(result[0].branch.as_deref(), Some("main"));
        assert_eq!(result[0].name, "main");

        assert_eq!(result[1].branch.as_deref(), Some("feature/auth"));
        assert_eq!(result[1].name, "feature/auth");
        assert_eq!(result[1].path, PathBuf::from("/Users/dev/my-repo-feature"));
    }

    #[test]
    fn test_parse_three_mixed_detached() {
        let result = parse_worktree_list(THREE_WORKTREES_DETACHED);
        assert_eq!(result.len(), 3);

        assert_eq!(result[0].branch.as_deref(), Some("main"));
        assert_eq!(result[1].branch.as_deref(), Some("experiment"));
        assert!(result[2].branch.is_none());
        assert_eq!(result[2].name, "hotfix");
        assert_eq!(result[2].head_hash, "2222222");
    }

    #[test]
    fn test_parse_extra_keys_ignored() {
        let result = parse_worktree_list(WORKTREE_EXTRA_KEYS);
        assert_eq!(result.len(), 1);

        let wt = &result[0];
        assert_eq!(wt.path, PathBuf::from("/Users/dev/my-repo"));
        assert_eq!(wt.head_hash, "abc1234");
    }

    #[test]
    fn test_parse_empty_input() {
        let result = parse_worktree_list("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_empty_path_returns_nothing() {
        let result = parse_worktree_list("HEAD abc1234");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_branch_strips_refs_heads() {
        let result = parse_worktree_list(THREE_WORKTREES_DETACHED);
        for wt in &result {
            if let Some(ref b) = wt.branch {
                assert!(
                    !b.starts_with("refs/heads/"),
                    "branch should be stripped: {b}"
                );
            }
        }
    }

    #[test]
    fn test_parse_dirty_clean() {
        assert!(!parse_dirty(""));
        assert!(!parse_dirty("  \n  "));
    }

    #[test]
    fn test_parse_dirty_with_changes() {
        assert!(parse_dirty(" M src/main.rs"));
        assert!(parse_dirty("?? new_file.txt"));
        assert!(parse_dirty("A  staged.rs\n M modified.rs"));
    }

    #[test]
    fn test_parse_rev_list_ahead() {
        let (ahead, behind) = parse_rev_list_count("3  0");
        assert_eq!(ahead, 3);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_parse_rev_list_behind() {
        let (ahead, behind) = parse_rev_list_count("0  5");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 5);
    }

    #[test]
    fn test_parse_rev_list_both() {
        let (ahead, behind) = parse_rev_list_count("2  7");
        assert_eq!(ahead, 2);
        assert_eq!(behind, 7);
    }

    #[test]
    fn test_parse_rev_list_synced() {
        let (ahead, behind) = parse_rev_list_count("0  0");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_parse_rev_list_empty() {
        let (ahead, behind) = parse_rev_list_count("");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_parse_rev_list_malformed() {
        let (ahead, behind) = parse_rev_list_count("not-a-number");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_parse_rev_list_single_value() {
        let (ahead, behind) = parse_rev_list_count("5");
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
    }

    #[test]
    fn test_status_clean() {
        let s = WorktreeStatus::clean();
        assert!(!s.is_dirty);
        assert_eq!(s.ahead, 0);
        assert_eq!(s.behind, 0);
        assert!(s.is_clean());
    }

    #[test]
    fn test_status_display_variants() {
        assert_eq!(WorktreeStatus::clean().to_string(), "clean");

        assert_eq!(
            WorktreeStatus {
                is_dirty: true,
                is_stale: false,
                ahead: 0,
                behind: 0
            }
            .to_string(),
            "dirty"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: false,
                is_stale: false,
                ahead: 3,
                behind: 0
            }
            .to_string(),
            "ahead 3"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: false,
                is_stale: false,
                ahead: 0,
                behind: 2
            }
            .to_string(),
            "behind 2"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: false,
                is_stale: false,
                ahead: 1,
                behind: 4
            }
            .to_string(),
            "ahead 1, behind 4"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: true,
                is_stale: false,
                ahead: 2,
                behind: 0
            }
            .to_string(),
            "dirty, ahead 2"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: true,
                is_stale: false,
                ahead: 0,
                behind: 3
            }
            .to_string(),
            "dirty, behind 3"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: true,
                is_stale: false,
                ahead: 1,
                behind: 1
            }
            .to_string(),
            "dirty, ahead 1, behind 1"
        );
    }

    #[test]
    fn test_is_clean_respects_all_fields() {
        assert!(WorktreeStatus {
            is_dirty: false,
            is_stale: false,
            ahead: 0,
            behind: 0
        }
        .is_clean());
        assert!(!WorktreeStatus {
            is_dirty: true,
            is_stale: false,
            ahead: 0,
            behind: 0
        }
        .is_clean());
        assert!(!WorktreeStatus {
            is_dirty: false,
            is_stale: false,
            ahead: 1,
            behind: 0
        }
        .is_clean());
        assert!(!WorktreeStatus {
            is_dirty: false,
            is_stale: false,
            ahead: 0,
            behind: 1
        }
        .is_clean());
    }

    #[test]
    fn test_derive_name_from_branch() {
        let name = derive_name(Path::new("/foo/bar"), Some("feature/auth"));
        assert_eq!(name, "feature/auth");
    }

    #[test]
    fn test_derive_name_from_path() {
        let name = derive_name(Path::new("/tmp/worktrees/hotfix"), None);
        assert_eq!(name, "hotfix");
    }

    #[test]
    fn test_derive_name_root_path() {
        let name = derive_name(Path::new("/"), None);
        assert_eq!(name, "unknown");
    }

    #[test]
    fn test_sanitize_simple_name() {
        assert_eq!(sanitize_branch_name("feature"), "feature");
    }

    #[test]
    fn test_sanitize_slash_to_hyphen() {
        assert_eq!(sanitize_branch_name("feature/auth"), "feature-auth");
    }

    #[test]
    fn test_sanitize_deep_nested() {
        assert_eq!(
            sanitize_branch_name("feature/user/auth-v2"),
            "feature-user-auth-v2"
        );
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(sanitize_branch_name("fix@bug!"), "fix-bug-");
    }

    #[test]
    fn test_sanitize_dots_underscores_hyphens_preserved() {
        assert_eq!(sanitize_branch_name("v2.0_hotfix-1"), "v2.0_hotfix-1");
    }

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize_branch_name(""), "");
    }

    #[test]
    fn test_infer_worktree_path() {
        let path = infer_worktree_path(Path::new("/src/api"), "feature/login");
        assert_eq!(path.unwrap(), PathBuf::from("/src/api-feature-login"));
    }

    #[test]
    fn test_infer_worktree_path_simple_branch() {
        let path = infer_worktree_path(Path::new("/Users/dev/my-repo"), "hotfix");
        assert_eq!(path.unwrap(), PathBuf::from("/Users/dev/my-repo-hotfix"));
    }

    #[test]
    fn test_infer_worktree_path_special_chars() {
        let path = infer_worktree_path(Path::new("/projects/web"), "fix/urgent-bug");
        assert_eq!(path.unwrap(), PathBuf::from("/projects/web-fix-urgent-bug"));
    }

    #[test]
    fn test_parse_prune_dry_run_empty() {
        assert!(parse_prune_dry_run("").is_empty());
        assert!(parse_prune_dry_run("nothing here\n").is_empty());
    }

    #[test]
    fn test_parse_prune_dry_run_single() {
        let out = "Removing worktree: /home/dev/proj-stale\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(result, vec!["/home/dev/proj-stale".to_owned()]);
    }

    #[test]
    fn test_parse_prune_dry_run_multiple() {
        let out = "Removing worktree: /a/b\n\
Removing worktree: /c/d\n\
Removing worktree: /e/f\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(
            result,
            vec!["/a/b".to_owned(), "/c/d".to_owned(), "/e/f".to_owned()]
        );
    }

    #[test]
    fn test_parse_prune_dry_run_ignores_other_lines() {
        let out = "Removing worktree: /only-path\n\
Some other message: not a removal\n\
Removing worktree: /second\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(result, vec!["/only-path".to_owned(), "/second".to_owned()]);
    }

    #[test]
    fn test_parse_prune_dry_run_paths_with_spaces() {
        let out = "Removing worktree: /home/user/my project\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(result, vec!["/home/user/my project".to_owned()]);
    }

    #[test]
    fn test_parse_prune_dry_run_single_quoted_path() {
        let out = "Removing worktree: '/home/user/my project'\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(result, vec!["/home/user/my project".to_owned()]);
    }

    #[test]
    fn test_parse_prune_dry_run_double_quoted_path() {
        let out = "Removing worktree: \"/home/user/my project\"\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(result, vec!["/home/user/my project".to_owned()]);
    }

    #[test]
    fn test_parse_prune_dry_run_mixed_quoted_and_plain() {
        let out = "Removing worktree: '/a b c'\n\
Removing worktree: /plain/path\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(result, vec!["/a b c".to_owned(), "/plain/path".to_owned()]);
    }

    #[test]
    fn test_parse_prune_dry_run_modern_relative_format() {
        let out =
            "Removing worktrees/test-rm-repo-stale2: gitdir file points to non-existent location\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(result, vec!["worktrees/test-rm-repo-stale2".to_owned()]);
    }

    #[test]
    fn test_parse_prune_dry_run_modern_relative_multiple() {
        let out = "Removing worktrees/a: first reason\n\
Removing worktrees/b: second reason\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(
            result,
            vec!["worktrees/a".to_owned(), "worktrees/b".to_owned()]
        );
    }

    #[test]
    fn test_parse_prune_dry_run_mixed_formats() {
        let out = "Removing worktree: /abs/path\n\
Removing worktrees/rel: missing gitdir\n";
        let result = parse_prune_dry_run(out);
        assert_eq!(
            result,
            vec!["/abs/path".to_owned(), "worktrees/rel".to_owned()]
        );
    }

    #[test]
    fn test_resolve_stale_absolute_path() {
        let path = resolve_stale_path("/abs/stale-dir", Path::new("/repo")).unwrap();
        assert_eq!(path, PathBuf::from("/abs/stale-dir"));
    }

    #[test]
    fn test_resolve_stale_relative_path_reads_gitdir() {
        // Set up a fake admin dir with a gitdir file pointing to a fake .git.
        let repo = "/tmp/resolve-test-repo";
        std::fs::create_dir_all(format!("{repo}/.git/worktrees/feature-x")).unwrap();
        std::fs::write(
            format!("{repo}/.git/worktrees/feature-x/gitdir"),
            "/tmp/resolve-test-repo-feature-x/.git",
        )
        .unwrap();

        let path = resolve_stale_path("worktrees/feature-x", Path::new(repo)).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/resolve-test-repo-feature-x"));

        std::fs::remove_dir_all("/tmp/resolve-test-repo").unwrap();
    }

    #[test]
    fn test_resolve_stale_relative_missing_gitdir_falls_back() {
        // A missing gitdir file should not error; it falls back to a displayable
        // path so `prune` can still print the entry and succeed.
        let result = resolve_stale_path("worktrees/missing", Path::new("/no/such/repo")).unwrap();
        assert_eq!(result, PathBuf::from("/no/such/repo/worktrees/missing"));
    }
}
