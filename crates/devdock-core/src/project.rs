//! `Project`: a folder the user docked in DevDock.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::detect::{Detected, ProjectKind};
use crate::paths;
use crate::run::CustomCommand;

/// Stable id: a hash of the normalised path, as 16 hex digits.
/// A string, not a number, because JavaScript can't hold a u64 exactly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub String);

impl ProjectId {
    pub fn for_path(path: &Path) -> Self {
        Self(format!("{:016x}", paths::stable_hash(&paths::key(path))))
    }
}

/// What is saved to `devdock.json`. New fields need `#[serde(default)]` so older
/// files still load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub kind: ProjectKind,
    #[serde(default)]
    pub is_git: bool,
    #[serde(default)]
    pub favorite: bool,
    /// Unix time in milliseconds of the last successful "open" action.
    #[serde(default)]
    pub last_opened: Option<u64>,
    #[serde(default)]
    pub added_at: u64,
    /// Commands the user added in the Run tab.
    #[serde(default)]
    pub commands: Vec<CustomCommand>,
    /// Ports learned from each command's output (command id -> port), used for the
    /// pre-launch conflict check.
    #[serde(default)]
    pub ports: std::collections::BTreeMap<String, u16>,
    /// Per command (command id -> prefs): port mode, saved port, port flag.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub run_prefs: std::collections::BTreeMap<String, crate::run::RunPrefs>,
    /// `shop.localhost`; `None` = derived from the name (see `run::serve`).
    #[serde(default)]
    pub domain: Option<String>,
    /// Open at `http://shop.localhost` (through DevDock's proxy) instead of
    /// `http://shop.localhost:4521`. `None` = follow Settings.
    #[serde(default)]
    pub hide_port: Option<bool>,
    /// The user's group (folder) this project is in, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<crate::groups::GroupId>,
    /// Recent agent/terminal launches, newest first (at most `KEEP_LAUNCHES`).
    #[serde(default)]
    pub launches: Vec<crate::agents::LaunchRecord>,
    /// Recent activity, newest first (at most `KEEP_ACTIVITY`).
    #[serde(default)]
    pub activity: Vec<crate::activity::Activity>,
}

impl Project {
    /// Builds a project for an already-normalised path. The name is the folder name;
    /// a drive root such as `D:\` has none, so the whole path is used.
    pub fn new(path: PathBuf, detected: &Detected, now: u64) -> Self {
        Self {
            id: ProjectId::for_path(&path),
            name: default_name(&path),
            kind: detected.kind,
            is_git: detected.is_git,
            favorite: false,
            last_opened: None,
            added_at: now,
            commands: Vec::new(),
            ports: Default::default(),
            run_prefs: Default::default(),
            domain: None,
            hide_port: None,
            group: None,
            launches: Vec::new(),
            activity: Vec::new(),
            path,
        }
    }

    pub fn apply_detection(&mut self, detected: &Detected) {
        self.kind = detected.kind;
        self.is_git = detected.is_git;
    }
}

pub fn default_name(path: &Path) -> String {
    match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Detected {
        Detected {
            kind: ProjectKind::Unknown,
            markers: vec![],
            is_git: false,
        }
    }

    #[test]
    fn name_defaults_to_folder_name() {
        let p = Project::new(PathBuf::from("D:\\dev\\devchrono"), &none(), 0);
        assert_eq!(p.name, "devchrono");
    }

    #[test]
    fn drive_root_uses_the_whole_path() {
        assert_eq!(default_name(Path::new("D:\\")), "D:\\");
    }

    #[test]
    fn id_is_the_same_for_two_spellings() {
        assert_eq!(
            ProjectId::for_path(Path::new("Q:\\Dev\\x\\")),
            ProjectId::for_path(Path::new("q:/dev/x"))
        );
        assert_eq!(ProjectId::for_path(Path::new("Q:\\a")).0.len(), 16);
    }

    #[test]
    fn old_files_without_new_fields_still_load() {
        let json = r#"{"id":"00ff","name":"api","path":"C:\\x\\api"}"#;
        let p: Project = serde_json::from_str(json).unwrap();
        assert_eq!(p.kind, ProjectKind::Unknown);
        assert!(!p.favorite);
        assert_eq!(p.last_opened, None);
    }

    #[test]
    fn serializes_camel_case() {
        let p = Project::new(PathBuf::from("C:\\x\\api"), &none(), 5);
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["name"], "api");
        assert_eq!(json["addedAt"], 5);
        assert_eq!(json["isGit"], false);
        assert!(json["id"].is_string());
    }
}
