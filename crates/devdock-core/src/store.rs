//! Saving and loading `devdock.json` (T2.1).
//!
//! Saves are atomic: write `devdock.json.tmp`, copy the current file to `.bak`, then
//! rename the temp file over it. A crash mid-save leaves the old file intact.
//! A damaged file is never overwritten: it is renamed to `.corrupt-<time>` first.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project::Project;

pub const VERSION: u32 = 1;
const FILE: &str = "devdock.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRoot {
    pub path: PathBuf,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub version: u32,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub scan_roots: Vec<ScanRoot>,
    #[serde(default)]
    pub settings: crate::prefs::Settings,
    #[serde(default)]
    pub shortcuts: Vec<crate::prefs::Shortcut>,
    #[serde(default)]
    pub workspaces: Vec<crate::workspace::Workspace>,
    #[serde(default)]
    pub groups: Vec<crate::groups::Group>,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            version: VERSION,
            projects: Vec::new(),
            scan_roots: Vec::new(),
            settings: Default::default(),
            shortcuts: Vec::new(),
            workspaces: Vec::new(),
            groups: Vec::new(),
        }
    }
}

/// Result of a load. `warning` is shown to the user once at startup.
#[derive(Debug, Default)]
pub struct Loaded {
    pub saved: Saved,
    pub warning: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("{} is not valid DevDock data: {source}", path.display())]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("{} was written by a newer DevDock (format {found})", path.display())]
    TooNew { path: PathBuf, found: u32 },
    #[error("could not save {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
}

#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn file(&self) -> PathBuf {
        self.dir.join(FILE)
    }

    fn sibling(&self, suffix: &str) -> PathBuf {
        self.dir.join(format!("{FILE}{suffix}"))
    }

    /// Never fails: on damage it falls back to `.bak`, then to empty, with a warning.
    pub fn load(&self) -> Loaded {
        let main = self.file();
        let bak = self.sibling(".bak");
        if !main.exists() && !bak.exists() {
            return Loaded::default();
        }

        let main_err = match read(&main) {
            Ok(saved) => {
                return Loaded {
                    saved,
                    warning: None,
                };
            }
            Err(err) => err,
        };
        // Damaged or from a newer DevDock: move it aside so the next save can't overwrite it.
        let kept = self.set_aside(&main);

        match read(&bak) {
            Ok(saved) => Loaded {
                saved,
                warning: Some(format!(
                    "{main_err}. DevDock restored the previous save from {}.{}",
                    bak.display(),
                    kept.map(|p| format!(" The damaged file was kept as {}.", p.display()))
                        .unwrap_or_default()
                )),
            },
            Err(_) => Loaded {
                saved: Saved::default(),
                warning: Some(format!(
                    "{main_err}. No usable backup was found, so DevDock started with an empty list.{}",
                    kept.map(|p| format!(" The damaged file was kept as {}.", p.display()))
                        .unwrap_or_default()
                )),
            },
        }
    }

    /// Renames a damaged file so the next save can't destroy it.
    fn set_aside(&self, path: &Path) -> Option<PathBuf> {
        if !path.exists() {
            return None;
        }
        let target = self.sibling(&format!(".corrupt-{}", crate::time::now_ms()));
        fs::rename(path, &target).ok().map(|()| target)
    }

    pub fn save(&self, saved: &Saved) -> Result<(), StoreError> {
        let main = self.file();
        let tmp = self.sibling(".tmp");
        let bak = self.sibling(".bak");
        let write_err = |source| StoreError::Write {
            path: main.clone(),
            source,
        };

        fs::create_dir_all(&self.dir).map_err(write_err)?;
        let json = serde_json::to_vec_pretty(saved).map_err(|e| write_err(e.into()))?;
        {
            let mut file = fs::File::create(&tmp).map_err(write_err)?;
            file.write_all(&json).map_err(write_err)?;
            file.sync_all().map_err(write_err)?;
        }
        if main.exists() {
            fs::copy(&main, &bak).map_err(write_err)?;
        }
        // On Windows `rename` replaces an existing target (MoveFileEx + REPLACE_EXISTING).
        fs::rename(&tmp, &main).map_err(write_err)
    }
}

/// Reads and validates a DevDock data file (also used by Settings > Import).
pub fn read_file(path: &Path) -> Result<Saved, StoreError> {
    read(path)
}

/// Writes a copy of the data for Settings > Export (not atomic: it's a copy).
pub fn export_file(saved: &Saved, path: &Path) -> Result<(), StoreError> {
    let write_err = |source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    };
    let json = serde_json::to_vec_pretty(saved).map_err(|e| write_err(e.into()))?;
    fs::write(path, json).map_err(write_err)
}

fn read(path: &Path) -> Result<Saved, StoreError> {
    let bytes = fs::read(path).map_err(|source| StoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let saved: Saved = serde_json::from_slice(&bytes).map_err(|source| StoreError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    if saved.version > VERSION {
        return Err(StoreError::TooNew {
            path: path.to_path_buf(),
            found: saved.version,
        });
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::detect;

    fn sample(dir: &Path) -> Saved {
        let mut p = Project::new(dir.join("api"), &detect(dir), 1);
        p.favorite = true;
        p.last_opened = Some(42);
        Saved {
            version: VERSION,
            projects: vec![p],
            scan_roots: vec![ScanRoot {
                path: dir.to_path_buf(),
                depth: 3,
            }],
            ..Saved::default()
        }
    }

    #[test]
    fn export_then_import_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let data = sample(dir.path());
        let file = dir.path().join("devdock-export.json");
        export_file(&data, &file).unwrap();
        assert_eq!(read_file(&file).unwrap(), data);
        fs::write(&file, b"{\"version\": 99}").unwrap();
        assert!(matches!(read_file(&file), Err(StoreError::TooNew { .. })));
    }

    #[test]
    fn missing_file_loads_empty_without_warning() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = Store::new(dir.path()).load();
        assert_eq!(loaded.saved, Saved::default());
        assert!(loaded.warning.is_none());
    }

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("DevDock"));
        let data = sample(dir.path());
        store.save(&data).unwrap();
        let loaded = store.load();
        assert_eq!(loaded.saved, data);
        assert!(loaded.warning.is_none());
        assert!(!store.sibling(".tmp").exists());
    }

    #[test]
    fn second_save_keeps_the_previous_one_as_bak() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let first = sample(dir.path());
        store.save(&first).unwrap();
        store.save(&Saved::default()).unwrap();
        let bak: Saved = serde_json::from_slice(&fs::read(store.sibling(".bak")).unwrap()).unwrap();
        assert_eq!(bak, first);
    }

    #[test]
    fn corrupt_file_falls_back_to_bak_and_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        let good = sample(dir.path());
        store.save(&good).unwrap();
        store.save(&good).unwrap(); // now .bak == good
        fs::write(store.file(), b"{ not json").unwrap();

        let loaded = store.load();
        assert_eq!(loaded.saved, good);
        let warning = loaded.warning.unwrap();
        assert!(warning.contains("restored the previous save"), "{warning}");
        let kept: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".corrupt-"))
            .collect();
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn corrupt_file_without_backup_starts_empty_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        fs::write(store.file(), b"garbage").unwrap();
        let loaded = store.load();
        assert!(loaded.saved.projects.is_empty());
        assert!(loaded.warning.unwrap().contains("empty list"));
    }

    #[test]
    fn newer_format_is_rejected_and_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path());
        fs::write(store.file(), br#"{"version": 99, "projects": []}"#).unwrap();
        let loaded = store.load();
        assert!(loaded.warning.unwrap().contains("newer DevDock"));
        assert!(
            !store.file().exists(),
            "original must be moved, not overwritten later"
        );
    }
}
