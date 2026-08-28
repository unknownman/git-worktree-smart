use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::error::{AppError, CandidateMatch};
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
        let candidates = substring_matches
            .iter()
            .map(|wt| candidate_from_wt(wt))
            .collect();
        return Err(AppError::MultipleWorktreesMatch {
            query: query.to_owned(),
            candidates,
        });
    }

    // 3. Path resolution: if the query *looks like* a path (an explicit `.`,
    // `..`, or a separator-containing string), canonicalize it to an absolute
    // path and find the worktree that encloses it. When run from a subdirectory,
    // the query canonicalizes to a nested path, so we match with `starts_with`
    // and select the longest matching worktree (if multiple nest) to pick the
    // most specific one.
    //
    // Guarding on `is_path_like` is essential: a plain word like `src` or `test`
    // should fall through to the fuzzy matcher below, even if a local folder of
    // that name happens to exist in the current directory. Only queries that
    // explicitly reference a path get canonicalized; otherwise a local directory
    // would silently shadow a same-named branch worktree.
    //
    // This also runs AFTER exact and substring matching on purpose: a query like
    // `main` must match a branch/worktree named `main` first, even if a local
    // folder also named `main` happens to exist in the current worktree.
    let is_path_like = query == "." || query == ".." || query.contains('/') || query.contains('\\');
    if is_path_like {
        if let Ok(query_canon) = dunce::canonicalize(query) {
            let mut best: Option<(usize, &WorktreeInfo)> = None;
            for wt in worktrees {
                if let Ok(wt_canon) = dunce::canonicalize(&wt.path) {
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
    }

    // 4. If the query is an absolute path and failed to resolve in the exact or
    // path-like stages above, do NOT fall through to fuzzy matching. Fuzzy
    // matching an absolute path against branch names is a safety hazard: e.g.
    // `wt rm /tmp/typo/path` could match a branch with similar characters and
    // delete the wrong worktree.
    if std::path::Path::new(query).is_absolute() {
        return Err(AppError::WorktreeNotFound {
            query: query.to_owned(),
        });
    }

    // 5. Fuzzy match
    // `SkimMatcherV2` treats spaces as literal characters, so a multi-word query
    // like `wt switch feature auth` (joined into "feature auth") would fail to
    // match `feature/auth`. Strip whitespace for fuzzy scoring only; exact and
    // substring matching above handle spaces correctly for real directory names.
    let fuzzy_query = query.replace(' ', "");
    let matcher = SkimMatcherV2::default();

    // Scores below this floor are too weak to trust as a match (e.g. a single
    // arbitrary matching character), so we refuse them rather than surfacing an
    // unrelated worktree.
    const MIN_FUZZY_SCORE: i64 = 40;

    let mut scored: Vec<(i64, &WorktreeInfo)> = worktrees
        .iter()
        .filter_map(|wt| {
            let name_score = matcher.fuzzy_match(&wt.name, &fuzzy_query);
            let branch_score = wt
                .branch
                .as_ref()
                .and_then(|b| matcher.fuzzy_match(b, &fuzzy_query));
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
        // A tie for the top score is ambiguous. Collect every candidate that
        // shares that exact top score so the user knows what to disambiguate.
        let top_score = scored[0].0;
        let candidates: Vec<CandidateMatch> = scored
            .iter()
            .filter(|(score, _)| *score == top_score)
            .map(|(_, wt)| candidate_from_wt(wt))
            .collect();
        return Err(AppError::MultipleWorktreesMatch {
            query: query.to_owned(),
            candidates,
        });
    }

    Ok(scored[0].1.clone())
}

/// Build a [`CandidateMatch`] from a worktree, capturing its name, branch, and
/// path so an ambiguous-match error can tell the user exactly what matched.
fn candidate_from_wt(wt: &WorktreeInfo) -> CandidateMatch {
    CandidateMatch {
        name: wt.name.clone(),
        branch: wt.branch.clone(),
        path: wt.path.clone(),
    }
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
            AppError::MultipleWorktreesMatch { query, candidates } => {
                assert_eq!(query, "log");
                assert_eq!(candidates.len(), 2);
                assert_eq!(candidates[0].name, "feature/login");
                assert_eq!(candidates[0].branch.as_deref(), Some("feature/login"));
                assert_eq!(
                    candidates[0].path,
                    PathBuf::from("/repos/project-feature/login")
                );
                assert_eq!(candidates[1].name, "feature/logout");
                assert_eq!(candidates[1].branch.as_deref(), Some("feature/logout"));
                assert_eq!(
                    candidates[1].path,
                    PathBuf::from("/repos/project-feature/logout")
                );
            }
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
            AppError::MultipleWorktreesMatch { query, candidates } => {
                assert_eq!(query, "f/l");
                assert_eq!(candidates.len(), 2);
                assert_eq!(candidates[0].name, "feature/login");
                assert_eq!(candidates[0].branch.as_deref(), Some("feature/login"));
                assert_eq!(
                    candidates[0].path,
                    PathBuf::from("/repos/project-feature/login")
                );
                assert_eq!(candidates[1].name, "feature/logout");
                assert_eq!(candidates[1].branch.as_deref(), Some("feature/logout"));
                assert_eq!(
                    candidates[1].path,
                    PathBuf::from("/repos/project-feature/logout")
                );
            }
            other => panic!("expected MultipleWorktreesMatch, got {other:?}"),
        }
    }

    #[test]
    fn test_fuzzy_match_ignores_spaces_in_query() {
        // The CLI can join a multi-word target into "feature auth". Because
        // `SkimMatcherV2` treats spaces literally, the fuzzy query must have its
        // whitespace stripped so "f auth" still matches "feature/auth".
        let worktrees = vec![
            worktree("main", Some("main")),
            worktree("feature/auth", Some("feature/auth")),
        ];
        let result = resolve_from_worktrees(&worktrees, "f auth").unwrap();
        assert_eq!(result.name, "feature/auth");
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
    fn test_absolute_path_never_falls_through_to_fuzzy_match() {
        // A missing absolute path must be rejected as WorktreeNotFound rather
        // than accidentally resolving to a branch via fuzzy matching. Otherwise
        // `wt rm /tmp/typo/path` could delete the wrong worktree.
        let result = resolve_from_worktrees(&mock_worktrees(), "/absolute/path/to/nowhere");
        assert!(
            matches!(result, Err(AppError::WorktreeNotFound { .. })),
            "expected WorktreeNotFound, got {result:?}"
        );
    }

    #[test]
    fn test_empty_worktrees() {
        let result = resolve_from_worktrees(&[], "main");
        assert!(result.is_err());
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

    /// Run the closure with the process's current directory temporarily set to
    /// `dir`, restoring the original directory afterwards even on panic. The
    /// path-resolution step canonicalizes the query relative to cwd, so this is
    /// required to reproduce "a local folder whose name equals a branch".
    ///
    /// Safety note: this briefly mutates the global cwd, but only this test
    /// relies on it (all other unit tests use absolute tempdir paths), and the
    /// integration tests run in a separate process, so this does not introduce
    /// cross-test interference.
    fn with_cwd<F: FnOnce()>(dir: &std::path::Path, f: F) {
        let original = std::env::current_dir().expect("read current dir");
        std::env::set_current_dir(dir).expect("change to temp cwd");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        std::env::set_current_dir(&original).expect("restore current dir");
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }

    #[test]
    fn test_exact_branch_match_beats_local_directory_path() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let cur_wt = dir.path().join("repo");
        let local = cur_wt.join("main");
        std::fs::create_dir_all(&local).expect("create local dir named like a branch");

        let main_wt = dir.path().join("main-wt");
        std::fs::create_dir_all(&main_wt).expect("create main worktree");

        let worktrees = vec![
            WorktreeInfo {
                path: cur_wt.clone(),
                name: "repo".to_owned(),
                branch: Some("repo".to_owned()),
                head_hash: "abc1234".to_owned(),
                head_msg: "msg".to_owned(),
                status: WorktreeStatus::clean(),
            },
            WorktreeInfo {
                path: main_wt.clone(),
                name: "main".to_owned(),
                branch: Some("main".to_owned()),
                head_hash: "abcd123".to_owned(),
                head_msg: "msg".to_owned(),
                status: WorktreeStatus::clean(),
            },
        ];

        // From inside the `repo` worktree there is a local folder named `main`.
        // Regardless, querying `main` must resolve to the worktree whose branch
        // is `main`, not to the enclosing worktree of the local directory —
        // exact/substring matching take precedence over path resolution.
        with_cwd(&cur_wt, || {
            let result = resolve_from_worktrees(&worktrees, "main").unwrap();
            assert_eq!(result.name, "main");
        });
    }

    #[test]
    fn test_plain_word_falls_through_to_fuzzy_even_with_local_dir() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let cur_wt = dir.path().join("repo");
        // A local folder named `src` exists inside the current worktree, so
        // `canonicalize("src")` would succeed from that directory.
        let local = cur_wt.join("src");
        std::fs::create_dir_all(&local).expect("create local src folder");

        let search_wt = dir.path().join("search-wt");
        std::fs::create_dir_all(&search_wt).expect("create search worktree");

        let worktrees = vec![
            WorktreeInfo {
                path: cur_wt.clone(),
                name: "repo".to_owned(),
                branch: Some("repo".to_owned()),
                head_hash: "abc1234".to_owned(),
                head_msg: "msg".to_owned(),
                status: WorktreeStatus::clean(),
            },
            WorktreeInfo {
                path: search_wt.clone(),
                name: "search".to_owned(),
                branch: Some("feature/search".to_owned()),
                head_hash: "abcd123".to_owned(),
                head_msg: "msg".to_owned(),
                status: WorktreeStatus::clean(),
            },
        ];

        // Because `src` is a plain word (no separators, not `.`/`..`), it is not
        // treated as a path. Even though a local `src` folder exists in the cwd,
        // the resolver must fall through to the fuzzy matcher and pick `search`
        // rather than short-circuiting to the enclosing worktree.
        with_cwd(&cur_wt, || {
            let result = resolve_from_worktrees(&worktrees, "src").unwrap();
            assert_eq!(result.name, "search");
        });
    }
}
