use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub name: String,
    pub branch: Option<String>,
    pub head_hash: String,
    pub head_msg: String,
    pub status: WorktreeStatus,
}

impl WorktreeInfo {
    pub fn display_branch(&self) -> &str {
        self.branch.as_deref().unwrap_or("detached HEAD")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeStatus {
    pub is_dirty: bool,
    pub ahead: u32,
    pub behind: u32,
}

impl WorktreeStatus {
    pub fn clean() -> Self {
        Self {
            is_dirty: false,
            ahead: 0,
            behind: 0,
        }
    }

    pub fn is_clean(&self) -> bool {
        !self.is_dirty && self.ahead == 0 && self.behind == 0
    }
}

impl fmt::Display for WorktreeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.is_dirty, self.ahead, self.behind) {
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

impl fmt::Display for WorktreeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) [{}]",
            self.path.display(),
            self.display_branch(),
            self.status
        )
    }
}
