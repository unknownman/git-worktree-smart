use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::command::run_git;
use crate::models::{WorktreeInfo, WorktreeStatus};

pub fn get_worktrees(verbose: bool) -> Result<Vec<WorktreeInfo>, AppError> {
    let output = run_git(&["worktree", "list", "--porcelain"], None, verbose)?;
    let mut worktrees = parse_worktree_list(&output);

    for wt in &mut worktrees {
        let msg = get_head_message(&wt.head_hash, verbose)?;
        wt.head_msg = msg;

        let status = get_worktree_status(&wt.path, verbose)?;
        wt.status = status;
    }

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

fn get_head_message(hash: &str, verbose: bool) -> Result<String, AppError> {
    let output = run_git(&["log", "-1", "--format=%s", hash], None, verbose)?;
    Ok(output)
}

pub fn get_worktree_status(cwd: &Path, verbose: bool) -> Result<WorktreeStatus, AppError> {
    let is_dirty = check_dirty(cwd, verbose)?;
    let (ahead, behind) = get_ahead_behind(cwd, verbose)?;

    Ok(WorktreeStatus {
        is_dirty,
        ahead,
        behind,
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
    let output = run_git(&["status", "--porcelain"], Some(cwd), verbose)?;
    Ok(parse_dirty(&output))
}

fn get_ahead_behind(cwd: &Path, verbose: bool) -> Result<(u32, u32), AppError> {
    let upstream_check = run_git(
        &["rev-parse", "--verify", "--quiet", "@{u}"],
        Some(cwd),
        verbose,
    );

    if upstream_check.is_err() {
        return Ok((0, 0));
    }

    let output = run_git(
        &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
        Some(cwd),
        verbose,
    )?;

    Ok(parse_rev_list_count(&output))
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
                ahead: 0,
                behind: 0
            }
            .to_string(),
            "dirty"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: false,
                ahead: 3,
                behind: 0
            }
            .to_string(),
            "ahead 3"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: false,
                ahead: 0,
                behind: 2
            }
            .to_string(),
            "behind 2"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: false,
                ahead: 1,
                behind: 4
            }
            .to_string(),
            "ahead 1, behind 4"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: true,
                ahead: 2,
                behind: 0
            }
            .to_string(),
            "dirty, ahead 2"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: true,
                ahead: 0,
                behind: 3
            }
            .to_string(),
            "dirty, behind 3"
        );

        assert_eq!(
            WorktreeStatus {
                is_dirty: true,
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
            ahead: 0,
            behind: 0
        }
        .is_clean());
        assert!(!WorktreeStatus {
            is_dirty: true,
            ahead: 0,
            behind: 0
        }
        .is_clean());
        assert!(!WorktreeStatus {
            is_dirty: false,
            ahead: 1,
            behind: 0
        }
        .is_clean());
        assert!(!WorktreeStatus {
            is_dirty: false,
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
}
