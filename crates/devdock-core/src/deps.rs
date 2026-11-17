//! Are a project's packages installed? Cheap file checks only (no package manager
//! runs), so the project list can show it for every project.
//!
//! Node projects: `node_modules` must exist, and the package manager's own marker
//! inside it must be newer than the lockfile. A lockfile changed after the last
//! install (a `git pull` that bumped dependencies) means "out of date".

use std::fs;
use std::path::Path;
use std::time::SystemTime;

use serde::Serialize;
use serde_json::Value;

use crate::run::spec::{PackageManager, package_manager};

/// The Run/Logs command id for installing packages.
pub const INSTALL_COMMAND: &str = "deps:install";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DepsState {
    Installed,
    /// Never installed: no `node_modules`.
    Missing,
    /// The lockfile changed after the last install.
    Outdated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepsStatus {
    pub manager: PackageManager,
    pub state: DepsState,
    /// What the Install button runs, e.g. `pnpm install`.
    pub command: String,
    /// The lockfile the check used, if any (`pnpm-lock.yaml`).
    pub lockfile: Option<&'static str>,
}

/// `None` when the folder has no packages to install (no `package.json`, or one
/// with no dependencies).
pub fn check(dir: &Path) -> Option<DepsStatus> {
    if !has_dependencies(&dir.join("package.json")) {
        return None;
    }
    let pm = package_manager(dir);
    let (lockfiles, marker): (&[&'static str], &str) = match pm {
        PackageManager::Npm => (&["package-lock.json"], "node_modules/.package-lock.json"),
        PackageManager::Pnpm => (&["pnpm-lock.yaml"], "node_modules/.modules.yaml"),
        PackageManager::Yarn => (&["yarn.lock"], "node_modules/.yarn-integrity"),
        PackageManager::Bun => (&["bun.lock", "bun.lockb"], "node_modules"),
    };
    let lockfile = lockfiles.iter().copied().find(|f| dir.join(f).is_file());
    // Yarn 2+ can install without node_modules (Plug'n'Play).
    let pnp = dir.join(".pnp.cjs").is_file();
    let installed_at = if pnp {
        modified(&dir.join(".yarn/install-state.gz"))
    } else if dir.join("node_modules").is_dir() {
        modified(&dir.join(marker)).or_else(|| modified(&dir.join("node_modules")))
    } else {
        None
    };
    let state = match (installed_at, lockfile.and_then(|f| modified(&dir.join(f)))) {
        (None, _) => DepsState::Missing,
        // Two seconds of slack: the install itself rewrites the lockfile.
        (Some(at), Some(lock)) if newer(lock, at, 2) => DepsState::Outdated,
        _ => DepsState::Installed,
    };
    Some(DepsStatus {
        manager: pm,
        state,
        command: format!("{} install", pm.program()),
        lockfile,
    })
}

fn has_dependencies(package_json: &Path) -> bool {
    let Ok(text) = fs::read_to_string(package_json) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    ["dependencies", "devDependencies", "optionalDependencies"]
        .iter()
        .any(|k| {
            json.get(k)
                .and_then(Value::as_object)
                .is_some_and(|m| !m.is_empty())
        })
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn newer(a: SystemTime, b: SystemTime, slack_secs: u64) -> bool {
    a.duration_since(b).is_ok_and(|d| d.as_secs() > slack_secs)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn set_time(path: &Path, t: SystemTime) {
        let f = fs::File::options().write(true).open(path).unwrap();
        f.set_modified(t).unwrap();
    }

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for (name, body) in files {
            let p = d.path().join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
        d
    }

    const PKG: &str = r#"{"dependencies":{"left-pad":"1.0.0"}}"#;

    #[test]
    fn no_dependencies_means_nothing_to_install() {
        assert!(check(project(&[]).path()).is_none());
        assert!(check(project(&[("package.json", r#"{"scripts":{}}"#)]).path()).is_none());
        assert!(check(project(&[("package.json", "{ broken")]).path()).is_none());
    }

    #[test]
    fn missing_installed_and_outdated() {
        let d = project(&[("package.json", PKG), ("pnpm-lock.yaml", "")]);
        let s = check(d.path()).unwrap();
        assert_eq!(s.state, DepsState::Missing);
        assert_eq!(s.command, "pnpm install");
        assert_eq!(s.lockfile, Some("pnpm-lock.yaml"));

        fs::create_dir_all(d.path().join("node_modules")).unwrap();
        fs::write(d.path().join("node_modules/.modules.yaml"), "").unwrap();
        let old = SystemTime::now() - Duration::from_secs(600);
        set_time(&d.path().join("pnpm-lock.yaml"), old);
        assert_eq!(check(d.path()).unwrap().state, DepsState::Installed);

        // The lockfile moves on (a pull bumped a dependency).
        set_time(&d.path().join("node_modules/.modules.yaml"), old);
        set_time(&d.path().join("pnpm-lock.yaml"), SystemTime::now());
        assert_eq!(check(d.path()).unwrap().state, DepsState::Outdated);
    }

    #[test]
    fn npm_without_a_lockfile_is_installed_once_node_modules_exists() {
        let d = project(&[("package.json", PKG), ("node_modules/x/index.js", "")]);
        let s = check(d.path()).unwrap();
        assert_eq!(s.state, DepsState::Installed);
        assert_eq!(s.command, "npm install");
        assert_eq!(s.lockfile, None);
    }
}
