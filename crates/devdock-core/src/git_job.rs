//! Refreshing git state for many projects off the UI thread (T3.5).
//! Up to `WORKERS` git processes run at once; the state lock is only held to claim
//! work and to store each result, never while git runs.

use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;

use crate::git::{self, Depth, Snapshot};
use crate::project::ProjectId;
use crate::state::{AppState, lock};
use crate::time::now_ms;

const WORKERS: usize = 4;

/// Refreshes `ids` (or every project when `ids` is empty). Calls `updated` from a
/// worker thread after each project's result is stored. Returns how many ran.
pub fn refresh(
    state: &Mutex<AppState>,
    ids: &[ProjectId],
    depth: Depth,
    updated: impl Fn(&ProjectId) + Sync,
) -> usize {
    let work: Vec<(ProjectId, PathBuf)> = lock(state).claim_git(ids, depth);
    let count = work.len();
    let queue = Mutex::new(work);

    thread::scope(|scope| {
        for _ in 0..WORKERS.min(count) {
            scope.spawn(|| {
                loop {
                    let next = queue.lock().unwrap_or_else(|p| p.into_inner()).pop();
                    let Some((id, path)) = next else { break };
                    match depth {
                        Depth::Status => {
                            let snap = Snapshot::from_result(git::status(&path), now_ms());
                            lock(state).git_mut().store_status(&id, snap);
                        }
                        Depth::Details => {
                            let snap = Snapshot::from_result(git::repo_info(&path), now_ms());
                            lock(state).git_mut().store_details(&id, snap);
                        }
                    }
                    updated(&id);
                }
            });
        }
    });
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Query;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn plain_folders_become_no_repo_and_missing_ones_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let gone = dir.path().join("gone");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&gone).unwrap();
        let state = Mutex::new(AppState::default());
        lock(&state).add(&a, 0).unwrap();
        lock(&state).add(&gone, 0).unwrap();
        std::fs::remove_dir(&gone).unwrap();
        lock(&state).refresh();

        let calls = AtomicUsize::new(0);
        let ran = refresh(&state, &[], Depth::Status, |_| {
            calls.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(ran, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let s = lock(&state);
        let view = serde_json::to_value(s.view(&Query::default())).unwrap();
        let states: Vec<_> = view["projects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["git"]["state"].as_str().unwrap_or("none").to_owned())
            .collect();
        assert!(states.contains(&"noRepo".to_owned()) || states.contains(&"error".to_owned()));
        assert!(
            states.contains(&"none".to_owned()),
            "missing folder was queried"
        );
    }
}
