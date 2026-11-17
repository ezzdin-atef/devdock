//! Finding projects under a scan root (T2.5a). Pure: no threads, no Tauri.
//! The caller decides what to do with each candidate via the `found` callback.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::detect::{Detected, detect};

pub const DEFAULT_DEPTH: usize = 3;
pub const IGNORE: [&str; 5] = ["node_modules", "target", ".next", "vendor", ".git"];

#[derive(Debug)]
pub struct Candidate {
    pub path: PathBuf,
    pub detected: Detected,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScanStats {
    /// Folders skipped because their name is on the ignore list.
    pub ignored: usize,
    /// Folders that could not be read (permissions, broken links).
    pub unreadable: usize,
}

/// Walks `root` up to `depth` levels below it. Once a folder is detected as a project,
/// its subfolders are not searched: a monorepo is one entry, not dozens.
pub fn scan(
    root: &Path,
    depth: usize,
    ignore: &[&str],
    mut found: impl FnMut(Candidate),
) -> ScanStats {
    let mut stats = ScanStats::default();
    let mut walker = WalkDir::new(root).max_depth(depth).into_iter();

    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                stats.unreadable += 1;
                continue;
            }
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if entry.depth() > 0 && ignore.iter().any(|i| name.eq_ignore_ascii_case(i)) {
            stats.ignored += 1;
            walker.skip_current_dir();
            continue;
        }

        let detected = detect(entry.path());
        if detected.is_project() {
            found(Candidate {
                path: entry.path().to_path_buf(),
                detected,
            });
            walker.skip_current_dir();
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "").unwrap();
    }

    fn names(root: &Path, depth: usize) -> (Vec<String>, ScanStats) {
        let mut out = Vec::new();
        let stats = scan(root, depth, &IGNORE, |c| {
            let rel = c
                .path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        });
        out.sort();
        (out, stats)
    }

    #[test]
    fn finds_projects_at_several_depths() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "shop/package.json");
        touch(d.path(), "client-a/api/Cargo.toml");
        touch(d.path(), "client-a/notes.txt");
        let (found, _) = names(d.path(), 3);
        assert_eq!(found, vec!["client-a/api", "shop"]);
    }

    #[test]
    fn does_not_descend_into_a_project() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "mono/package.json");
        touch(d.path(), "mono/packages/ui/package.json");
        let (found, _) = names(d.path(), 4);
        assert_eq!(found, vec!["mono"]);
    }

    #[test]
    fn skips_ignored_folders_and_counts_them() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "work/node_modules/left-pad/package.json");
        touch(d.path(), "work/target/x/Cargo.toml");
        let (found, stats) = names(d.path(), 4);
        assert!(found.is_empty());
        assert_eq!(stats.ignored, 2);
    }

    #[test]
    fn respects_depth() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "a/b/c/package.json");
        assert!(names(d.path(), 2).0.is_empty());
        assert_eq!(names(d.path(), 3).0, vec!["a/b/c"]);
    }

    #[test]
    fn root_that_is_a_project_is_one_result() {
        let d = tempfile::tempdir().unwrap();
        touch(d.path(), "package.json");
        touch(d.path(), "sub/Cargo.toml");
        assert_eq!(names(d.path(), 3).0, vec![""]);
    }

    #[test]
    fn missing_root_reports_unreadable() {
        let (found, stats) = names(Path::new("Z:\\no\\such\\root"), 3);
        assert!(found.is_empty());
        assert_eq!(stats.unreadable, 1);
    }

    #[test]
    fn fifteen_projects_each_found_once() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..15 {
            touch(d.path(), &format!("group{}/p{i}/package.json", i % 3));
        }
        let (found, _) = names(d.path(), 3);
        assert_eq!(found.len(), 15);
        let mut dedup = found.clone();
        dedup.dedup();
        assert_eq!(dedup, found);
    }
}
