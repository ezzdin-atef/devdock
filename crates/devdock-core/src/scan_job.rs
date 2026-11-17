//! Running a scan against shared state (T2.5b). Called on a worker thread by
//! `commands`; kept free of Tauri so it can be tested directly.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;

use crate::scan::{self, IGNORE};
use crate::state::{AppState, lock};
use crate::store::ScanRoot;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub root: PathBuf,
    pub found: usize,
    pub added: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub roots: usize,
    pub added: usize,
    pub duplicates: usize,
    pub ignored: usize,
    pub unreadable: usize,
    pub missing_roots: Vec<PathBuf>,
}

/// Scans each root and merges results into `state`. The lock is taken once per
/// candidate, never during file-system walking, so the UI stays responsive.
pub fn run(
    state: &Mutex<AppState>,
    roots: &[ScanRoot],
    now: u64,
    mut progress: impl FnMut(&ScanProgress),
) -> ScanSummary {
    let mut summary = ScanSummary {
        roots: roots.len(),
        ..ScanSummary::default()
    };
    for root in roots {
        if !root.path.is_dir() {
            summary.missing_roots.push(root.path.clone());
            continue;
        }
        let mut tick = ScanProgress {
            root: root.path.clone(),
            found: 0,
            added: 0,
        };
        let stats = scan::scan(&root.path, root.depth, &IGNORE, |candidate| {
            tick.found += 1;
            if lock(state).add_scanned(candidate, now) {
                tick.added += 1;
                summary.added += 1;
            } else {
                summary.duplicates += 1;
            }
            progress(&tick);
        });
        summary.ignored += stats.ignored;
        summary.unreadable += stats.unreadable;
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{Library, Query};
    use std::fs;
    use std::path::Path;

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "").unwrap();
    }

    #[test]
    fn adds_new_counts_duplicates_and_marks_new() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "shop/package.json");
        touch(d.path(), "api/Cargo.toml");
        // Not a project itself, so the scanner looks inside and skips node_modules.
        touch(d.path(), "misc/node_modules/left-pad/package.json");

        let state = Mutex::new(AppState::default());
        lock(&state).add(&d.path().join("shop"), 0).unwrap();
        let roots = [ScanRoot {
            path: d.path().to_path_buf(),
            depth: 3,
        }];
        let mut ticks = 0;
        let summary = run(&state, &roots, 1, |_| ticks += 1);

        assert_eq!(
            (summary.added, summary.duplicates, summary.ignored),
            (1, 1, 1)
        );
        assert_eq!(ticks, 2);
        let s = lock(&state);
        let q = Query {
            library: Library::New,
            ..Query::default()
        };
        let new: Vec<_> = s
            .view(&q)
            .projects
            .iter()
            .map(|p| p.project.name.clone())
            .collect();
        assert_eq!(new, vec!["api"]);
    }

    #[test]
    fn second_scan_adds_nothing() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "a/package.json");
        let state = Mutex::new(AppState::default());
        let roots = [ScanRoot {
            path: d.path().to_path_buf(),
            depth: 2,
        }];
        assert_eq!(run(&state, &roots, 1, |_| {}).added, 1);
        let again = run(&state, &roots, 2, |_| {});
        assert_eq!((again.added, again.duplicates), (0, 1));
    }

    #[test]
    fn missing_root_is_reported_not_fatal() {
        let state = Mutex::new(AppState::default());
        let roots = [ScanRoot {
            path: "Z:\\gone".into(),
            depth: 3,
        }];
        let summary = run(&state, &roots, 0, |_| {});
        assert_eq!(summary.missing_roots, vec![PathBuf::from("Z:\\gone")]);
    }
}
