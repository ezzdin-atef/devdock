//! Project-type detection from marker files (T2.4).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Ordered by display priority: the first matching rule decides the kind.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum ProjectKind {
    NextJs,
    Node,
    Rust,
    Php,
    Compose,
    #[default]
    Unknown,
}

impl ProjectKind {
    /// Short uppercase label, as shown in lists.
    pub fn label(self) -> &'static str {
        match self {
            ProjectKind::NextJs => "NEXT.JS",
            ProjectKind::Node => "NODE",
            ProjectKind::Rust => "RUST",
            ProjectKind::Php => "PHP",
            ProjectKind::Compose => "COMPOSE",
            ProjectKind::Unknown => "FOLDER",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub kind: ProjectKind,
    /// Marker file names that were found, for display and debugging.
    pub markers: Vec<&'static str>,
    pub is_git: bool,
}

impl Detected {
    /// A folder counts as a project if any rule matched, including a bare Git repo.
    pub fn is_project(&self) -> bool {
        self.kind != ProjectKind::Unknown || self.is_git
    }
}

const NEXT_CONFIGS: [&str; 4] = [
    "next.config.js",
    "next.config.mjs",
    "next.config.ts",
    "next.config.cjs",
];
const COMPOSE_FILES: [&str; 4] = [
    "docker-compose.yml",
    "docker-compose.yaml",
    "compose.yml",
    "compose.yaml",
];

pub fn detect(dir: &Path) -> Detected {
    let has = |name: &str| dir.join(name).is_file();
    let first_of = |names: &[&'static str]| names.iter().copied().find(|n| has(n));

    let mut markers = Vec::new();
    let package = has("package.json").then_some("package.json");
    let next = first_of(&NEXT_CONFIGS);
    let cargo = has("Cargo.toml").then_some("Cargo.toml");
    let composer = has("composer.json").then_some("composer.json");
    let compose = first_of(&COMPOSE_FILES);
    markers.extend(
        [package, next, cargo, composer, compose]
            .into_iter()
            .flatten(),
    );

    // `.git` is a folder in a normal repo and a file in a worktree or submodule.
    let is_git = dir.join(".git").exists();
    if is_git {
        markers.push(".git");
    }

    let kind = match (package, next, cargo, composer, compose) {
        (Some(_), Some(_), ..) => ProjectKind::NextJs,
        (Some(_), None, ..) => ProjectKind::Node,
        (None, _, Some(_), ..) => ProjectKind::Rust,
        (None, _, None, Some(_), _) => ProjectKind::Php,
        (None, _, None, None, Some(_)) => ProjectKind::Compose,
        _ => ProjectKind::Unknown,
    };

    Detected {
        kind,
        markers,
        is_git,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn folder_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            fs::write(dir.path().join(f), "").unwrap();
        }
        dir
    }

    #[test]
    fn next_needs_package_json_and_config() {
        let d = folder_with(&["package.json", "next.config.mjs"]);
        assert_eq!(detect(d.path()).kind, ProjectKind::NextJs);
    }

    #[test]
    fn next_config_alone_is_not_next() {
        let d = folder_with(&["next.config.js"]);
        assert_eq!(detect(d.path()).kind, ProjectKind::Unknown);
    }

    #[test]
    fn package_json_is_node() {
        let d = folder_with(&["package.json"]);
        assert_eq!(detect(d.path()).kind, ProjectKind::Node);
    }

    #[test]
    fn cargo_toml_is_rust() {
        let d = folder_with(&["Cargo.toml"]);
        assert_eq!(detect(d.path()).kind, ProjectKind::Rust);
    }

    #[test]
    fn composer_json_is_php() {
        let d = folder_with(&["composer.json"]);
        assert_eq!(detect(d.path()).kind, ProjectKind::Php);
    }

    #[test]
    fn compose_file_is_compose() {
        let d = folder_with(&["docker-compose.yml"]);
        assert_eq!(detect(d.path()).kind, ProjectKind::Compose);
    }

    #[test]
    fn node_wins_over_compose_but_keeps_both_markers() {
        let d = folder_with(&["package.json", "compose.yaml"]);
        let r = detect(d.path());
        assert_eq!(r.kind, ProjectKind::Node);
        assert_eq!(r.markers, vec!["package.json", "compose.yaml"]);
    }

    #[test]
    fn git_dir_is_detected() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join(".git")).unwrap();
        let r = detect(d.path());
        assert!(r.is_git);
        assert!(r.is_project());
        assert_eq!(r.kind, ProjectKind::Unknown);
    }

    #[test]
    fn git_file_marks_a_worktree_as_git() {
        let d = folder_with(&[".git"]);
        assert!(detect(d.path()).is_git);
    }

    #[test]
    fn empty_folder_is_not_a_project() {
        let d = tempfile::tempdir().unwrap();
        assert!(!detect(d.path()).is_project());
    }

    #[test]
    fn marker_directory_named_like_a_file_does_not_count() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("package.json")).unwrap();
        assert_eq!(detect(d.path()).kind, ProjectKind::Unknown);
    }
}
