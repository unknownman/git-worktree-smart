use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::error::AppError;
use crate::git::parse::get_worktrees;
use crate::models::WorktreeInfo;
use crate::Context;

/// Fetch the worktree list and resolve `query` to a single worktree.
pub fn resolve_worktree(ctx: &Context, query: &str) -> Result<WorktreeInfo, AppError> {
    let worktrees = get_worktrees(ctx.verbose)?;
    resolve_from_worktrees(&worktrees, query)
}

/// Pure resolution logic: given an in-memory list of worktrees and a query
/// string, resolve it to exactly one worktree via path, exact, substring, or
/// fuzzy matching. This has no I/O, so it is directly unit-testable.
pub fn resolve_from_worktrees(
    worktrees: &[WorktreeInfo],
    query: &str,
) -> Result<WorktreeInfo, AppError> {
    if worktrees.is_empty() {
        return Err(AppError::WorktreeNotFound {
            query: query.to_owned(),
        });
    }

    // 0. Path resolution: if the query can be canonicalized to an absolute
    // path (e.g. `.`, `..`, or a relative dir), match it against the
    // canonicalized worktree paths so `wt path .` or `wt rm .` resolve to the
    // worktree you are currently standing in.
    if let Ok(query_canon) = std::fs::canonicalize(query) {
        for wt in worktrees {
            if let Ok(wt_canon) = std::fs::canonicalize(&wt.path) {
                if wt_canon == query_canon {
                    return Ok(wt.clone());
                }
            }
        }
    }

    // 1. Exact match
    for wt in worktrees {
        if wt.name == query
            || wt.branch.as_deref() == Some(query)
            || wt.path.to_string_lossy() == query
        {
            return Ok(wt.clone());
        }
    }

    // 2. Substring match (unique)
    let substring_matches: Vec<&WorktreeInfo> = worktrees
        .iter()
        .filter(|wt| {
            wt.name.contains(query) || wt.branch.as_deref().is_some_and(|b| b.contains(query))
        })
        .collect();

    if substring_matches.len() == 1 {
        return Ok(substring_matches[0].clone());
    }

    if substring_matches.len() > 1 {
        return Err(AppError::MultipleWorktreesMatch {
            query: query.to_owned(),
        });
    }

    // 3. Fuzzy match
    let matcher = SkimMatcherV2::default();

    let mut scored: Vec<(i64, &WorktreeInfo)> = worktrees
        .iter()
        .filter_map(|wt| {
            let name_score = matcher.fuzzy_match(&wt.name, query);
            let branch_score = wt
                .branch
                .as_ref()
                .and_then(|b| matcher.fuzzy_match(b, query));
            let best = match (name_score, branch_score) {
                (Some(n), Some(b)) => Some(n.max(b)),
                (Some(n), None) => Some(n),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            best.map(|s| (s, wt))
        })
        .collect();

    if scored.is_empty() {
        return Err(AppError::WorktreeNotFound {
            query: query.to_owned(),
        });
    }

    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));

    if scored.len() > 1 && scored[0].0 == scored[1].0 {
        return Err(AppError::MultipleWorktreesMatch {
            query: query.to_owned(),
        });
    }

    if scored[0].0 < 60 {
        return Err(AppError::WorktreeNotFound {
            query: query.to_owned(),
        });
    }

    Ok(scored[0].1.clone())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::models::WorktreeStatus;

    fn worktree(name: &str, branch: Option<&str>) -> WorktreeInfo {
        WorktreeInfo {
            path: PathBuf::from(format!("/repos/project-{name}")),
            name: name.to_owned(),
            branch: branch.map(|b| b.to_owned()),
            head_hash: "abc1234".to_owned(),
            head_msg: "latest commit".to_owned(),
            status: WorktreeStatus::clean(),
        }
    }

    fn mock_worktrees() -> Vec<WorktreeInfo> {
        vec![
            worktree("main", Some("main")),
            worktree("feature/login", Some("feature/login")),
            worktree("feature/logout", Some("feature/logout")),
            worktree("hotfix", Some("hotfix/bug")),
        ]
    }

    #[test]
    fn test_exact_match_name() {
        let result = resolve_from_worktrees(&mock_worktrees(), "main").unwrap();
        assert_eq!(result.name, "main");
    }

    #[test]
    fn test_exact_match_branch() {
        let result = resolve_from_worktrees(&mock_worktrees(), "feature/login").unwrap();
        assert_eq!(result.name, "feature/login");
    }

    #[test]
    fn test_exact_match_path() {
        let result = resolve_from_worktrees(&mock_worktrees(), "/repos/project-hotfix").unwrap();
        assert_eq!(result.name, "hotfix");
    }

    #[test]
    fn test_substring_unique_match() {
        let result = resolve_from_worktrees(&mock_worktrees(), "logi").unwrap();
        assert_eq!(result.name, "feature/login");
    }

    #[test]
    fn test_substring_unique_match_branch() {
        let result = resolve_from_worktrees(&mock_worktrees(), "hotfix/bug").unwrap();
        assert_eq!(result.name, "hotfix");
    }

    #[test]
    fn test_substring_ambiguous_returns_multiple_error() {
        let result = resolve_from_worktrees(&mock_worktrees(), "log");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::MultipleWorktreesMatch { query } => assert_eq!(query, "log"),
            other => panic!("expected MultipleWorktreesMatch, got {other:?}"),
        }
    }

    #[test]
    fn test_fuzzy_match() {
        let result = resolve_from_worktrees(&mock_worktrees(), "f/logi").unwrap();
        assert_eq!(result.name, "feature/login");
    }

    #[test]
    fn test_fuzzy_match_ambiguous() {
        let result = resolve_from_worktrees(&mock_worktrees(), "f/l");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::MultipleWorktreesMatch { query } => assert_eq!(query, "f/l"),
            other => panic!("expected MultipleWorktreesMatch, got {other:?}"),
        }
    }

    #[test]
    fn test_not_found() {
        let result = resolve_from_worktrees(&mock_worktrees(), "zzz");
        assert!(result.is_err());
        match result.unwrap_err() {
            AppError::WorktreeNotFound { query } => assert_eq!(query, "zzz"),
            other => panic!("expected WorktreeNotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_empty_worktrees() {
        let result = resolve_from_worktrees(&[], "main");
        assert!(result.is_err());
    }
}
