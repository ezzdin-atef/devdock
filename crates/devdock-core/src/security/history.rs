//! Saved scan reports: `<data dir>\security\<project id>\<started at>.json`.
//! The newest few are kept, so the next scan can say what changed.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::model::{Report, Summary};

const KEEP: usize = 10;

pub struct History {
    root: PathBuf,
}

impl History {
    /// `data_dir` is DevDock's data folder; reports go in its `security` folder.
    pub fn new(data_dir: &Path) -> Self {
        History {
            root: data_dir.join("security"),
        }
    }

    fn project_dir(&self, project_id: &str) -> PathBuf {
        // Project ids are hex hashes; anything else is refused rather than joined.
        let safe: String = project_id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        self.root.join(safe)
    }

    /// Report files of one project, newest first.
    fn files(&self, project_id: &str) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(self.project_dir(project_id)) else {
            return Vec::new();
        };
        let mut files: Vec<(u64, PathBuf)> = entries
            .flatten()
            .map(|e| e.path())
            .filter_map(|p| {
                let stamp = p.file_stem()?.to_str()?.parse::<u64>().ok()?;
                (p.extension()? == "json").then_some((stamp, p))
            })
            .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        files.into_iter().map(|(_, p)| p).collect()
    }

    pub fn save(&self, report: &Report) -> io::Result<()> {
        let dir = self.project_dir(&report.project_id);
        fs::create_dir_all(&dir)?;
        let text = serde_json::to_string(report).map_err(io::Error::other)?;
        let file = dir.join(format!("{}.json", report.started_at));
        let tmp = file.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &file)?;
        for old in self.files(&report.project_id).into_iter().skip(KEEP) {
            let _ = fs::remove_file(old);
        }
        Ok(())
    }

    pub fn latest(&self, project_id: &str) -> Option<Report> {
        self.files(project_id)
            .into_iter()
            .find_map(|f| serde_json::from_str(&fs::read_to_string(f).ok()?).ok())
    }

    /// The newest report's summary for every project that has one.
    pub fn summaries(&self) -> HashMap<String, Summary> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return HashMap::new();
        };
        entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let id = e.file_name().to_str()?.to_owned();
                let report = self.latest(&id)?;
                Some((id, report.summary()))
            })
            .collect()
    }

    pub fn forget(&self, project_id: &str) -> io::Result<()> {
        match fs::remove_dir_all(self.project_dir(project_id)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::model::{Counts, ScanOptions};

    fn report(id: &str, at: u64) -> Report {
        Report {
            version: 1,
            project_id: id.into(),
            project_name: "x".into(),
            path: "C:\\x".into(),
            started_at: at,
            finished_at: at + 1,
            options: ScanOptions::default(),
            tools: Vec::new(),
            findings: Vec::new(),
            counts: Counts {
                high: usize::try_from(at).unwrap(),
                ..Counts::default()
            },
            upgrades: Vec::new(),
            previous_at: None,
            resolved: Vec::new(),
            cancelled: false,
        }
    }

    #[test]
    fn keeps_the_newest_and_reads_them_back() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::new(dir.path());
        assert!(h.latest("abc").is_none());
        for at in 1..=12 {
            h.save(&report("abc", at)).unwrap();
        }
        assert_eq!(h.files("abc").len(), KEEP);
        assert_eq!(h.latest("abc").unwrap().started_at, 12);
        h.save(&report("def", 3)).unwrap();
        let all = h.summaries();
        assert_eq!(all.len(), 2);
        assert_eq!(all["abc"].counts.high, 12);
        h.forget("abc").unwrap();
        assert!(h.latest("abc").is_none());
        // Path tricks are stripped, not followed.
        assert_eq!(
            h.project_dir("..\\..\\x"),
            dir.path().join("security").join("x")
        );
    }
}
