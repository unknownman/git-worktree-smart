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
    // path (e.g. `.`, `..`, or a relative dir), find the worktree that
    // encloses it. When run from a subdirectory, the query canonicalizes to a
    // nested path, so we match with `starts_with` and select the longest
    // matching worktree (if multiple nest) to pick the most specific one.
    if let Ok(query_canon) = std::fs::canonicalize(query) {
        let mut best: Option<(usize, &WorktreeInfo)> = None;
        for wt in worktrees {
            if let Ok(wt_canon) = std::fs::canonicalize(&wt.path) {
                if query_canon.starts_with(&wt_canon) {
                    // Prefer the worktree whose root is deepest along the path.
                    if let Some((best_len, _)) = best {
                        if wt_canon.as_os_str().len() > best_len {
                            best = Some((wt_canon.as_os_str().len(), wt));
                        }
                    } else {
                        best = Some((wt_canon.as_os_str().len(), wt));
                    }
                }
            }
        }
        if let Some((_, wt)) = best {
            return Ok(wt.clone());
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

    // 2. Substring match (unique), case-insensitive.
    let lower_query = query.to_lowercase();
    let substring_matches: Vec<&WorktreeInfo> = worktrees
        .iter()
        .filter(|wt| {
            wt.name.to_lowercase().contains(&lower_query)
                || wt
                    .branch
                    .as_deref()
                    .is_some_and(|b| b.to_lowercase().contains(&lower_query))
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

    // Scores below this floor are too weak to trust as a match (e.g. a single
    // arbitrary matching character), so we refuse them rather than surfacing an
    // unrelated worktree.
    const MIN_FUZZY_SCORE: i64 = 40;

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
            // Only keep candidates whose best score clears the confidence floor.
            best.filter(|&s| s >= MIN_FUZZY_SCORE).map(|s| (s, wt))
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

    Ok(scored[0].1.clone())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::models::WorktreeStatus;

    /// Restores the process working directory when dropped so that temporarily
    /// changing cwd inside a test never leaks into other (parallel) tests.
    struct ChdirGuard {
        original: PathBuf,
    }

    impl ChdirGuard {
        fn new(target: &std::path::Path) -> std::io::Result<Self> {
            let original = std::env::current_dir()?;
            std::env::set_current_dir(target)?;
            Ok(Self { original })
        }
    }

    impl Drop for ChdirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

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
    fn test_low_scoring_fuzzy_query_returns_not_found() {
        // "xyz" shares no meaningful prefix/sequence with any mock worktree
        // name or branch; any fuzzy score it produces must fall below the
        // confidence floor and be rejected.
        let result = resolve_from_worktrees(&mock_worktrees(), "xyz");
        match result {
            Err(AppError::WorktreeNotFound { query }) => assert_eq!(query, "xyz"),
            other => panic!("expected WorktreeNotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_weak_fuzzy_query_returns_not_found() {
        // "li" is not a substring of any mock name or branch (so it bypasses
        // substring matching), but it does score against "feature/login" at ~34
        // — below the confidence floor. It must be rejected as not found rather
        // than falsely matching the worktree.
        let result = resolve_from_worktrees(&mock_worktrees(), "li");
        assert!(
            matches!(result, Err(AppError::WorktreeNotFound { .. })),
            "expected WorktreeNotFound, got {result:?}"
        );
    }

    #[test]
    fn test_random_low_relevance_query_returns_not_found() {
        let result = resolve_from_worktrees(&mock_worktrees(), "qzxv");
        assert!(
            matches!(result, Err(AppError::WorktreeNotFound { .. })),
            "expected WorktreeNotFound, got {result:?}"
        );
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

    #[test]
    fn test_path_resolves_dot_to_enclosing_worktree() {
        // Real directories so canonicalization succeeds and the path step runs.
        let dir = tempfile::tempdir().expect("create tempdir");
        let wt_dir = dir.path().join("project-feat");
        std::fs::create_dir_all(&wt_dir).expect("create wt dir");

        let worktrees = vec![WorktreeInfo {
            path: wt_dir.clone(),
            name: "feat".to_owned(),
            branch: Some("feat".to_owned()),
            head_hash: "abc1234".to_owned(),
            head_msg: "msg".to_owned(),
            status: WorktreeStatus::clean(),
        }];

        // Temporarily change cwd into the worktree so `.` canonicalizes to it,
        // then restore the original cwd so parallel tests are unaffected.
        let cwd_guard = ChdirGuard::new(&wt_dir).expect("chdir");

        let result = resolve_from_worktrees(&worktrees, ".").unwrap();
        assert_eq!(result.name, "feat");

        drop(cwd_guard);

        // Also verify the absolute path resolves directly.
        let result = resolve_from_worktrees(&worktrees, wt_dir.to_string_lossy().as_ref()).unwrap();
        assert_eq!(result.name, "feat");
    }

    #[test]
    fn test_path_resolves_subdirectory_to_enclosing_worktree() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let wt_dir = dir.path().join("project-feat");
        let sub = wt_dir.join("src").join("nested");
        std::fs::create_dir_all(&sub).expect("create subdir");

        let worktrees = vec![WorktreeInfo {
            path: wt_dir.clone(),
            name: "feat".to_owned(),
            branch: Some("feat".to_owned()),
            head_hash: "abc1234".to_owned(),
            head_msg: "msg".to_owned(),
            status: WorktreeStatus::clean(),
        }];

        // Query is a canonicalized path *inside* the worktree; it must resolve
        // to the enclosing worktree (starts_with), not require equality.
        let result = resolve_from_worktrees(&worktrees, sub.to_string_lossy().as_ref()).unwrap();
        assert_eq!(result.name, "feat");
    }

    #[test]
    fn test_path_nested_worktrees_prefers_deepest_match() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let outer_dir = dir.path().join("outer");
        let inner_dir = outer_dir.join("inner");
        std::fs::create_dir_all(&inner_dir).expect("create dirs");

        let worktrees = vec![
            WorktreeInfo {
                path: inner_dir.clone(),
                name: "inner".to_owned(),
                branch: Some("inner".to_owned()),
                head_hash: "abc1234".to_owned(),
                head_msg: "msg".to_owned(),
                status: WorktreeStatus::clean(),
            },
            WorktreeInfo {
                path: outer_dir.clone(),
                name: "outer".to_owned(),
                branch: Some("outer".to_owned()),
                head_hash: "abc1234".to_owned(),
                head_msg: "msg".to_owned(),
                status: WorktreeStatus::clean(),
            },
        ];

        let result = resolve_from_worktrees(&worktrees, inner_dir.to_string_lossy().as_ref());
        assert_eq!(result.unwrap().name, "inner");
    }
}
