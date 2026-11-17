//! Git state via the git CLI (M3). Reading never writes to a repository; the only
//! changes are the explicit Git-tab actions in `ops` (switch, pull, push...).

mod cache;
mod facts;
pub mod ops;
mod run;
mod status;

use std::time::Duration;

use serde::Serialize;

use crate::error::ErrorDto;

pub use cache::{Depth, GitCache};
pub use facts::{LastCommit, Remote, RepoInfo, Worktree, repo_info, status};
pub use run::{DEFAULT_TIMEOUT, git};
pub use status::{GitStatus, parse as parse_status};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git wasn't found on PATH")]
    NotInstalled,
    #[error("not inside a git repository")]
    NotARepo,
    #[error("git refuses to read this repository because another user owns it")]
    DubiousOwnership,
    #[error("git took longer than {} s", .0.as_secs())]
    Timeout(Duration),
    #[error("{0}")]
    Failed(String),
}

/// A cached git result as the UI sees it. "Not a repo" is a normal state, not an error.
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Snapshot<T> {
    Repo { data: T, refreshed_at: u64 },
    NoRepo { refreshed_at: u64 },
    Error { error: ErrorDto, refreshed_at: u64 },
}

impl<T> Snapshot<T> {
    pub fn from_result(result: Result<T, GitError>, now: u64) -> Self {
        match result {
            Ok(data) => Snapshot::Repo {
                data,
                refreshed_at: now,
            },
            Err(GitError::NotARepo) => Snapshot::NoRepo { refreshed_at: now },
            Err(err) => Snapshot::Error {
                error: err.into(),
                refreshed_at: now,
            },
        }
    }

    pub fn data(&self) -> Option<&T> {
        match self {
            Snapshot::Repo { data, .. } => Some(data),
            _ => None,
        }
    }

    pub fn map<U>(&self, f: impl FnOnce(&T) -> U) -> Snapshot<U> {
        match self {
            Snapshot::Repo { data, refreshed_at } => Snapshot::Repo {
                data: f(data),
                refreshed_at: *refreshed_at,
            },
            Snapshot::NoRepo { refreshed_at } => Snapshot::NoRepo {
                refreshed_at: *refreshed_at,
            },
            Snapshot::Error {
                error,
                refreshed_at,
            } => Snapshot::Error {
                error: error.clone(),
                refreshed_at: *refreshed_at,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_a_repo_is_a_state_not_an_error() {
        let s: Snapshot<u8> = Snapshot::from_result(Err(GitError::NotARepo), 5);
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["state"], "noRepo");
        assert_eq!(v["refreshedAt"], 5);
    }

    #[test]
    fn repo_snapshot_serializes_with_data() {
        let s = Snapshot::from_result(Ok(GitStatus::default()), 9);
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["state"], "repo");
        assert_eq!(v["data"]["changedFiles"], 0);
    }

    #[test]
    fn errors_carry_user_copy() {
        let s: Snapshot<u8> = Snapshot::from_result(Err(GitError::NotInstalled), 1);
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["state"], "error");
        assert_eq!(v["error"]["kind"], "gitMissing");
    }
}
