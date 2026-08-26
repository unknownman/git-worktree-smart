use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: String,
    pub status: WorktreeStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeStatus {
    pub dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

impl WorktreeStatus {
    pub fn clean() -> Self {
        Self {
            dirty: false,
            ahead: 0,
            behind: 0,
        }
    }

    pub fn is_clean(&self) -> bool {
        !self.dirty && self.ahead == 0 && self.behind == 0
    }
}

impl fmt::Display for WorktreeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.dirty, self.ahead, self.behind) {
            (false, 0, 0) => write!(f, "clean"),
            (true, 0, 0) => write!(f, "dirty"),
            (false, a, 0) => write!(f, "ahead {a}"),
            (false, 0, b) => write!(f, "behind {b}"),
            (false, a, b) => write!(f, "ahead {a}, behind {b}"),
            (true, a, 0) => write!(f, "dirty, ahead {a}"),
            (true, 0, b) => write!(f, "dirty, behind {b}"),
            (true, a, b) => write!(f, "dirty, ahead {a}, behind {b}"),
        }
    }
}

impl fmt::Display for Worktree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let branch = self.branch.as_deref().unwrap_or("detached HEAD");
        write!(f, "{} ({}) [{}]", self.path.display(), branch, self.status)
    }
}
