//! Domain state owned by Rust. Svelte only sees views of it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;

use crate::agents::{AgentId, KEEP_LAUNCHES, LaunchRecord, LaunchSpec, TerminalKind};
use crate::detect::detect;
use crate::error::ProjectError;
use crate::git::{Depth, GitCache, GitStatus, Snapshot};
use crate::library::{self, Counts, Flags, Query};
use crate::paths;
use crate::prefs::{SLOTS, Settings, Shortcut, valid_editor};
use crate::project::{Project, ProjectId};
use crate::run::{CommandId, CustomCommand, Discovered, discover, validate_custom};
use crate::scan::Candidate;
use crate::store::{Saved, ScanRoot, VERSION};
use crate::workspace::{
    AfterStart, RestartPolicy, Service, ServiceSource, Workspace, WorkspaceId, start_order,
};

#[derive(Debug, Default)]
pub struct AppState {
    projects: Vec<Project>,
    scan_roots: Vec<ScanRoot>,
    /// Folders not found on the last check (load, window focus, relocate).
    missing: HashSet<ProjectId>,
    /// Added by the most recent scan. Session only.
    new_ids: HashSet<ProjectId>,
    scanning: bool,
    git: GitCache,
    settings: Settings,
    shortcuts: Vec<Shortcut>,
    workspaces: Vec<Workspace>,
}

/// A poisoned lock only means another thread panicked mid-update; the data is still
/// usable, so recover instead of failing every later call.
pub fn lock(state: &Mutex<AppState>) -> MutexGuard<'_, AppState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView<'a> {
    #[serde(flatten)]
    pub project: &'a Project,
    pub missing: bool,
    pub is_new: bool,
    /// Last known git status; absent until the first refresh finishes.
    pub git: Option<&'a Snapshot<GitStatus>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView<'a> {
    pub projects: Vec<ProjectView<'a>>,
    pub counts: Counts,
    pub scan_roots: &'a [ScanRoot],
    pub scanning: bool,
}

impl AppState {
    pub fn from_saved(saved: Saved) -> Self {
        let mut state = Self {
            scan_roots: saved.scan_roots,
            settings: saved.settings,
            shortcuts: saved.shortcuts,
            workspaces: saved.workspaces,
            ..Self::default()
        };
        // Re-derive ids so a file edited by hand can't hold two copies of one folder.
        for mut p in saved.projects {
            p.id = ProjectId::for_path(&p.path);
            if state.find(&p.id).is_none() {
                state.projects.push(p);
            }
        }
        state.refresh();
        state
    }

    pub fn to_saved(&self) -> Saved {
        Saved {
            version: VERSION,
            projects: self.projects.clone(),
            scan_roots: self.scan_roots.clone(),
            settings: self.settings.clone(),
            shortcuts: self.shortcuts.clone(),
            workspaces: self.workspaces.clone(),
        }
    }

    pub fn view(&self, q: &Query) -> LibraryView<'_> {
        let flags = Flags {
            missing: &self.missing,
            new: &self.new_ids,
        };
        LibraryView {
            projects: library::filter(&self.projects, q, &flags)
                .into_iter()
                .map(|p| self.view_of(p))
                .collect(),
            counts: library::counts(&self.projects, q, &flags),
            scan_roots: &self.scan_roots,
            scanning: self.scanning,
        }
    }

    pub fn view_of<'a>(&'a self, project: &'a Project) -> ProjectView<'a> {
        ProjectView {
            missing: self.missing.contains(&project.id),
            is_new: self.new_ids.contains(&project.id),
            git: self.git.status(&project.id),
            project,
        }
    }

    /// Re-checks which folders exist and re-detects the type of those that do.
    /// Returns true if anything that gets saved changed.
    pub fn refresh(&mut self) -> bool {
        let mut changed = false;
        self.missing.clear();
        for p in &mut self.projects {
            if p.path.is_dir() {
                let detected = detect(&p.path);
                if (p.kind, p.is_git) != (detected.kind, detected.is_git) {
                    p.apply_detection(&detected);
                    changed = true;
                }
            } else {
                self.missing.insert(p.id.clone());
            }
        }
        changed
    }

    fn find(&self, id: &ProjectId) -> Option<usize> {
        self.projects.iter().position(|p| &p.id == id)
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn set_settings(&mut self, settings: Settings) -> Result<(), ProjectError> {
        if !valid_editor(&settings.editor) {
            return Err(ProjectError::InvalidCommand(format!(
                "{} isn't an editor DevDock knows.",
                settings.editor
            )));
        }
        self.settings = settings;
        Ok(())
    }

    /// Resolves an optional subfolder inside a project into a launch spec.
    pub fn launch_spec(
        &self,
        id: &ProjectId,
        agent: Option<AgentId>,
        terminal: TerminalKind,
        subdir: Option<&str>,
    ) -> Result<LaunchSpec, ProjectError> {
        let project = self.get(id)?;
        let subdir = validate_custom(&project.path, "launch", "launch", subdir)
            .map_err(ProjectError::InvalidCommand)?;
        Ok(LaunchSpec {
            terminal,
            cwd: subdir.map_or_else(|| project.path.clone(), |s| project.path.join(s)),
            program: agent.map(|a| a.command().to_owned()),
        })
    }

    pub fn record_activity(
        &mut self,
        id: &ProjectId,
        entry: crate::activity::Activity,
    ) -> Result<(), ProjectError> {
        crate::activity::push(&mut self.get_mut(id)?.activity, entry);
        Ok(())
    }

    pub fn record_launch(
        &mut self,
        id: &ProjectId,
        record: LaunchRecord,
    ) -> Result<(), ProjectError> {
        let project = self.get_mut(id)?;
        project.launches.insert(0, record);
        project.launches.truncate(KEEP_LAUNCHES);
        Ok(())
    }

    pub fn shortcuts(&self) -> &[Shortcut] {
        &self.shortcuts
    }

    /// Binds a slot, replacing whatever used it.
    pub fn set_shortcut(&mut self, shortcut: Shortcut) -> Result<(), ProjectError> {
        if !SLOTS.contains(&shortcut.slot) {
            return Err(ProjectError::InvalidCommand(
                "Shortcuts use Ctrl+Alt+1 to 9.".into(),
            ));
        }
        let project = self.get(&shortcut.project_id)?;
        validate_custom(
            &project.path,
            "launch",
            "launch",
            shortcut.subdir.as_deref(),
        )
        .map_err(ProjectError::InvalidCommand)?;
        self.shortcuts.retain(|s| s.slot != shortcut.slot);
        self.shortcuts.push(shortcut);
        self.shortcuts.sort_by_key(|s| s.slot);
        Ok(())
    }

    pub fn remove_shortcut(&mut self, slot: u8) {
        self.shortcuts.retain(|s| s.slot != slot);
    }

    pub fn shortcut(&self, slot: u8) -> Option<&Shortcut> {
        self.shortcuts.iter().find(|s| s.slot == slot)
    }

    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn workspace(&self, id: &WorkspaceId) -> Result<&Workspace, ProjectError> {
        self.workspaces
            .iter()
            .find(|w| &w.id == id)
            .ok_or_else(|| ProjectError::InvalidCommand("That workspace no longer exists.".into()))
    }

    fn workspace_mut(&mut self, id: &WorkspaceId) -> Result<&mut Workspace, ProjectError> {
        self.workspaces
            .iter_mut()
            .find(|w| &w.id == id)
            .ok_or_else(|| ProjectError::InvalidCommand("That workspace no longer exists.".into()))
    }

    pub fn create_workspace(&mut self, name: &str, now: u64) -> Result<WorkspaceId, ProjectError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ProjectError::EmptyName);
        }
        let id = WorkspaceId(format!(
            "{:016x}",
            paths::stable_hash(&format!("{name}{now}"))
        ));
        self.workspaces.push(Workspace {
            id: id.clone(),
            name: name.to_owned(),
            services: Vec::new(),
            restart: RestartPolicy::default(),
            after_start: AfterStart::default(),
        });
        Ok(id)
    }

    pub fn rename_workspace(&mut self, id: &WorkspaceId, name: &str) -> Result<(), ProjectError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ProjectError::EmptyName);
        }
        self.workspace_mut(id)?.name = name.to_owned();
        Ok(())
    }

    pub fn delete_workspace(&mut self, id: &WorkspaceId) {
        self.workspaces.retain(|w| &w.id != id);
    }

    /// Checks a service against the projects it refers to. `worktrees` are the
    /// project's Git worktree folders (the app reads them via git).
    pub fn validate_service(
        &self,
        svc: &Service,
        worktrees: &[PathBuf],
    ) -> Result<(), ProjectError> {
        let invalid = |m: String| Err(ProjectError::InvalidCommand(m));
        if svc.name.trim().is_empty() {
            return invalid("Give the service a name.".into());
        }
        let project = self.get(svc.source.project_id())?;
        match &svc.source {
            ServiceSource::Command {
                command_id,
                worktree,
                ..
            } => {
                let known = self
                    .commands(&project.id)?
                    .commands
                    .iter()
                    .any(|c| &c.id == command_id);
                if !known {
                    return invalid(format!("{} has no command {}.", project.name, command_id.0));
                }
                if let Some(w) = worktree
                    && !worktrees.iter().any(|t| paths::key(t) == paths::key(w))
                {
                    return invalid(format!(
                        "{} is not a worktree of {}.",
                        w.display(),
                        project.name
                    ));
                }
            }
            ServiceSource::Compose { file, service, .. } => {
                let rel = Path::new(file);
                let escapes = rel.is_absolute()
                    || rel.components().any(|c| {
                        matches!(
                            c,
                            std::path::Component::ParentDir | std::path::Component::Prefix(_)
                        )
                    });
                if escapes {
                    return invalid(format!("{file} must be a file inside {}.", project.name));
                }
                if !project.path.join(file).is_file() {
                    return invalid(format!("{file} doesn't exist in {}.", project.name));
                }
                let plain = !service.is_empty()
                    && service
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
                if !plain {
                    return invalid(format!("{service} isn't a Compose service name."));
                }
            }
        }
        if let Some(crate::workspace::Health::Http { url, .. }) = &svc.health {
            crate::workspace::health::parse_http_url(url).map_err(ProjectError::InvalidCommand)?;
        }
        Ok(())
    }

    /// Adds or replaces (same id) a service, keeping the workspace startable.
    pub fn upsert_service(&mut self, id: &WorkspaceId, svc: Service) -> Result<(), ProjectError> {
        let ws = self.workspace_mut(id)?;
        let mut services = ws.services.clone();
        match services.iter_mut().find(|s| s.id == svc.id) {
            Some(existing) => *existing = svc,
            None => services.push(svc),
        }
        start_order(&services).map_err(|e| ProjectError::InvalidCommand(e.to_string()))?;
        ws.services = services;
        Ok(())
    }

    pub fn remove_service(
        &mut self,
        id: &WorkspaceId,
        service_id: &str,
    ) -> Result<(), ProjectError> {
        let ws = self.workspace_mut(id)?;
        ws.services.retain(|s| s.id != service_id);
        for s in &mut ws.services {
            s.depends_on.retain(|d| d != service_id);
        }
        Ok(())
    }

    pub fn set_workspace_rules(
        &mut self,
        id: &WorkspaceId,
        restart: RestartPolicy,
        after_start: AfterStart,
    ) -> Result<(), ProjectError> {
        if restart.max > 20 || restart.window_secs == 0 {
            return Err(ProjectError::InvalidCommand(
                "Restarts: up to 20 within a window of at least 1 second.".into(),
            ));
        }
        let ws = self.workspace_mut(id)?;
        ws.restart = restart;
        ws.after_start = after_start;
        Ok(())
    }

    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    /// Remembers the port a command listens on. Returns true if it changed.
    pub fn learn_port(&mut self, id: &ProjectId, command: &CommandId, port: u16) -> bool {
        let Ok(project) = self.get_mut(id) else {
            return false;
        };
        project.ports.insert(command.0.clone(), port) != Some(port)
    }

    pub fn known_port(&self, id: &ProjectId, command: &CommandId) -> Option<u16> {
        self.get(id).ok()?.ports.get(&command.0).copied()
    }

    pub fn git(&self) -> &GitCache {
        &self.git
    }

    pub fn git_mut(&mut self) -> &mut GitCache {
        &mut self.git
    }

    /// Claims git refresh work for `ids` (all projects when empty), skipping missing
    /// folders and refreshes already running. Returns what the caller should run.
    pub fn claim_git(&mut self, ids: &[ProjectId], depth: Depth) -> Vec<(ProjectId, PathBuf)> {
        let wanted: Vec<ProjectId> = if ids.is_empty() {
            self.projects.iter().map(|p| p.id.clone()).collect()
        } else {
            ids.to_vec()
        };
        let mut work = Vec::new();
        for id in wanted {
            let Some(i) = self.find(&id) else { continue };
            if self.missing.contains(&id) || !self.git.claim(&id, depth) {
                continue;
            }
            work.push((id, self.projects[i].path.clone()));
        }
        work
    }

    pub fn get(&self, id: &ProjectId) -> Result<&Project, ProjectError> {
        self.find(id)
            .map(|i| &self.projects[i])
            .ok_or_else(|| ProjectError::UnknownId(id.clone()))
    }

    fn get_mut(&mut self, id: &ProjectId) -> Result<&mut Project, ProjectError> {
        let i = self
            .find(id)
            .ok_or_else(|| ProjectError::UnknownId(id.clone()))?;
        Ok(&mut self.projects[i])
    }

    /// Checks a folder chosen by the user and returns its normalised path.
    fn checked_folder(path: &Path) -> Result<PathBuf, ProjectError> {
        if !path.exists() {
            return Err(ProjectError::NotFound(path.to_path_buf()));
        }
        if !path.is_dir() {
            return Err(ProjectError::NotADirectory(path.to_path_buf()));
        }
        Ok(paths::normalize(path))
    }

    /// Adds a folder picked by the user. Any folder is allowed, project markers or not.
    pub fn add(&mut self, path: &Path, now: u64) -> Result<&Project, ProjectError> {
        let path = Self::checked_folder(path)?;
        let id = ProjectId::for_path(&path);
        if let Ok(existing) = self.get(&id) {
            return Err(ProjectError::Duplicate(existing.path.clone()));
        }
        self.projects
            .push(Project::new(path.clone(), &detect(&path), now));
        self.get(&id)
    }

    /// Adds a scan result unless it is already docked. Returns true if it was added.
    pub fn add_scanned(&mut self, candidate: Candidate, now: u64) -> bool {
        let path = paths::normalize(&candidate.path);
        let id = ProjectId::for_path(&path);
        if self.find(&id).is_some() {
            return false;
        }
        self.projects
            .push(Project::new(path, &candidate.detected, now));
        self.new_ids.insert(id);
        true
    }

    /// Forgets a project. The folder on disk is never touched.
    pub fn remove(&mut self, id: &ProjectId) -> Result<Project, ProjectError> {
        let i = self
            .find(id)
            .ok_or_else(|| ProjectError::UnknownId(id.clone()))?;
        self.missing.remove(id);
        self.new_ids.remove(id);
        self.git.forget(id);
        self.shortcuts.retain(|s| &s.project_id != id);
        Ok(self.projects.remove(i))
    }

    pub fn rename(&mut self, id: &ProjectId, name: &str) -> Result<(), ProjectError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ProjectError::EmptyName);
        }
        self.get_mut(id)?.name = name.to_owned();
        Ok(())
    }

    pub fn set_favorite(&mut self, id: &ProjectId, favorite: bool) -> Result<(), ProjectError> {
        self.get_mut(id)?.favorite = favorite;
        Ok(())
    }

    /// Clears the "new from scan" mark once the project has been looked at
    /// (selected in the list). Returns true if it was marked.
    pub fn mark_seen(&mut self, id: &ProjectId) -> bool {
        self.new_ids.remove(id)
    }

    pub fn touch_opened(&mut self, id: &ProjectId, now: u64) -> Result<(), ProjectError> {
        self.get_mut(id)?.last_opened = Some(now);
        self.new_ids.remove(id);
        Ok(())
    }

    /// Points a (usually missing) project at a new folder, keeping its name, favorite
    /// and history. The id follows the path, so it changes.
    pub fn relocate(&mut self, id: &ProjectId, path: &Path) -> Result<ProjectId, ProjectError> {
        let path = Self::checked_folder(path)?;
        let new_id = ProjectId::for_path(&path);
        if &new_id != id
            && let Ok(other) = self.get(&new_id)
        {
            return Err(ProjectError::Duplicate(other.path.clone()));
        }
        let detected = detect(&path);
        let project = self.get_mut(id)?;
        project.path = path;
        project.id = new_id.clone();
        project.apply_detection(&detected);
        self.missing.remove(id);
        self.git.forget(id);
        if self.new_ids.remove(id) {
            self.new_ids.insert(new_id.clone());
        }
        Ok(new_id)
    }

    /// Scripts, detected defaults and saved custom commands for a project.
    pub fn commands(&self, id: &ProjectId) -> Result<Discovered, ProjectError> {
        let p = self.get(id)?;
        Ok(discover(&p.path, &p.commands))
    }

    pub fn add_command(
        &mut self,
        id: &ProjectId,
        name: &str,
        command_line: &str,
        subdir: Option<&str>,
        now: u64,
    ) -> Result<CommandId, ProjectError> {
        let project = self.get_mut(id)?;
        let subdir = validate_custom(&project.path, name, command_line, subdir)
            .map_err(ProjectError::InvalidCommand)?;
        let short = format!("{:08x}", paths::stable_hash(&format!("{name}{now}")) as u32);
        project.commands.push(CustomCommand {
            id: short.clone(),
            name: name.trim().to_owned(),
            command_line: command_line.trim().to_owned(),
            subdir,
        });
        Ok(CommandId(format!("custom:{short}")))
    }

    pub fn remove_command(
        &mut self,
        id: &ProjectId,
        command: &CommandId,
    ) -> Result<(), ProjectError> {
        let project = self.get_mut(id)?;
        let short = command.0.strip_prefix("custom:").unwrap_or(&command.0);
        let before = project.commands.len();
        project.commands.retain(|c| c.id != short);
        if project.commands.len() == before {
            return Err(ProjectError::UnknownCommand(command.0.clone()));
        }
        Ok(())
    }

    pub fn scan_roots(&self) -> &[ScanRoot] {
        &self.scan_roots
    }

    pub fn add_scan_root(&mut self, path: &Path, depth: usize) -> Result<ScanRoot, ProjectError> {
        let path = Self::checked_folder(path)?;
        let key = paths::key(&path);
        if let Some(existing) = self.scan_roots.iter().find(|r| paths::key(&r.path) == key) {
            return Err(ProjectError::ScanRootExists(existing.path.clone()));
        }
        let root = ScanRoot { path, depth };
        self.scan_roots.push(root.clone());
        Ok(root)
    }

    pub fn set_scan_root_depth(&mut self, path: &Path, depth: usize) -> Result<(), ProjectError> {
        if !(1..=6).contains(&depth) {
            return Err(ProjectError::InvalidCommand(
                "Scan depth is 1 to 6 folders.".into(),
            ));
        }
        let key = paths::key(path);
        let root = self
            .scan_roots
            .iter_mut()
            .find(|r| paths::key(&r.path) == key)
            .ok_or_else(|| ProjectError::ScanRootUnknown(path.to_path_buf()))?;
        root.depth = depth;
        Ok(())
    }

    /// Replaces everything with imported data (Settings > Import).
    pub fn replace_with(&mut self, saved: Saved) {
        *self = AppState::from_saved(saved);
    }

    pub fn remove_scan_root(&mut self, path: &Path) -> Result<(), ProjectError> {
        let key = paths::key(path);
        let before = self.scan_roots.len();
        self.scan_roots.retain(|r| paths::key(&r.path) != key);
        if self.scan_roots.len() == before {
            return Err(ProjectError::ScanRootUnknown(path.to_path_buf()));
        }
        Ok(())
    }

    /// Marks a scan as running. Only one scan runs at a time.
    pub fn begin_scan(&mut self) -> Result<(), ProjectError> {
        if self.scanning {
            return Err(ProjectError::ScanRunning);
        }
        self.scanning = true;
        self.new_ids.clear();
        Ok(())
    }

    pub fn end_scan(&mut self) {
        self.scanning = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn state_with(dirs: &[&Path]) -> AppState {
        let mut s = AppState::default();
        for d in dirs {
            s.add(d, 1).unwrap();
        }
        s
    }

    #[test]
    fn selecting_a_scanned_project_clears_its_new_mark() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = AppState::default();
        let candidate = crate::scan::Candidate {
            path: dir.path().to_path_buf(),
            detected: detect(dir.path()),
        };
        assert!(s.add_scanned(candidate, 1));
        let id = s.projects[0].id.clone();
        assert!(s.new_ids.contains(&id));

        assert!(s.mark_seen(&id));
        assert!(!s.new_ids.contains(&id));
        assert!(!s.mark_seen(&id), "second call is a no-op");
    }

    #[test]
    fn add_normalises_and_detects() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let mut s = AppState::default();
        let p = s.add(dir.path(), 7).unwrap();
        assert_eq!(p.kind, crate::detect::ProjectKind::Rust);
        assert_eq!(p.added_at, 7);
        assert!(!p.path.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn duplicate_ignores_case_and_trailing_slash_and_reports_existing_path() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = state_with(&[dir.path()]);
        let existing = s.projects[0].path.clone();
        let again = format!("{}\\", dir.path().display()).to_uppercase();
        let err = s.add(Path::new(&again), 2).unwrap_err();
        assert!(matches!(err, ProjectError::Duplicate(ref p) if *p == existing));
        assert_eq!(s.projects.len(), 1);
    }

    #[test]
    fn missing_folder_and_file_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.txt");
        fs::write(&file, "").unwrap();
        let mut s = AppState::default();
        assert!(matches!(
            s.add(Path::new("Z:\\nope"), 0),
            Err(ProjectError::NotFound(_))
        ));
        assert!(matches!(
            s.add(&file, 0),
            Err(ProjectError::NotADirectory(_))
        ));
    }

    #[test]
    fn rename_trims_and_rejects_empty() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = state_with(&[dir.path()]);
        let id = s.projects[0].id.clone();
        s.rename(&id, "  Wallet API ").unwrap();
        assert_eq!(s.get(&id).unwrap().name, "Wallet API");
        assert!(matches!(s.rename(&id, "   "), Err(ProjectError::EmptyName)));
    }

    #[test]
    fn remove_forgets_the_project_but_not_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = state_with(&[dir.path()]);
        let id = s.projects[0].id.clone();
        s.remove(&id).unwrap();
        assert!(s.projects.is_empty());
        assert!(dir.path().is_dir());
    }

    #[test]
    fn refresh_flags_missing_folders() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone");
        fs::create_dir(&gone).unwrap();
        let mut s = state_with(&[&gone]);
        let id = s.projects[0].id.clone();
        fs::remove_dir(&gone).unwrap();
        s.refresh();
        assert!(s.view_of(s.get(&id).unwrap()).missing);
    }

    #[test]
    fn relocate_keeps_metadata_and_moves_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old");
        let new = dir.path().join("new");
        fs::create_dir(&old).unwrap();
        fs::create_dir(&new).unwrap();
        let mut s = state_with(&[&old]);
        let id = s.projects[0].id.clone();
        s.rename(&id, "Shop").unwrap();
        s.set_favorite(&id, true).unwrap();
        fs::remove_dir(&old).unwrap();
        s.refresh();

        let new_id = s.relocate(&id, &new).unwrap();
        let p = s.get(&new_id).unwrap();
        assert_eq!((p.name.as_str(), p.favorite), ("Shop", true));
        assert!(!s.view_of(p).missing);
        assert!(s.get(&id).is_err());
    }

    #[test]
    fn relocate_onto_another_project_is_a_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        let mut s = state_with(&[&a, &b]);
        let id_a = s.projects[0].id.clone();
        assert!(matches!(
            s.relocate(&id_a, &b),
            Err(ProjectError::Duplicate(_))
        ));
    }

    #[test]
    fn saved_round_trip_drops_hand_made_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let s = state_with(&[dir.path()]);
        let mut saved = s.to_saved();
        let mut copy = saved.projects[0].clone();
        copy.path = PathBuf::from(format!("{}\\", dir.path().display()).to_uppercase());
        copy.id = ProjectId("bogus".into());
        saved.projects.push(copy);
        assert_eq!(AppState::from_saved(saved).projects.len(), 1);
    }

    #[test]
    fn only_one_scan_at_a_time() {
        let mut s = AppState::default();
        s.begin_scan().unwrap();
        assert!(matches!(s.begin_scan(), Err(ProjectError::ScanRunning)));
        s.end_scan();
        s.begin_scan().unwrap();
    }

    #[test]
    fn scan_roots_are_deduplicated_and_removable() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = AppState::default();
        s.add_scan_root(dir.path(), 3).unwrap();
        let again = format!("{}\\", dir.path().display()).to_lowercase();
        assert!(matches!(
            s.add_scan_root(Path::new(&again), 3),
            Err(ProjectError::ScanRootExists(_))
        ));
        s.remove_scan_root(Path::new(&again)).unwrap();
        assert!(s.scan_roots().is_empty());
    }

    #[test]
    fn custom_commands_are_saved_listed_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("api")).unwrap();
        let mut s = state_with(&[dir.path()]);
        let id = s.projects[0].id.clone();

        let cmd = s
            .add_command(&id, " API ", " cargo run ", Some("api"), 1)
            .unwrap();
        let listed = s.commands(&id).unwrap().commands;
        let found = listed.iter().find(|c| c.id == cmd).unwrap();
        assert_eq!(
            (found.name.as_str(), found.command_line.as_str()),
            ("API", "cargo run")
        );
        assert_eq!(found.cwd, s.get(&id).unwrap().path.join("api"));
        // Saved with the project, so it survives a restart.
        assert_eq!(s.to_saved().projects[0].commands.len(), 1);

        assert!(matches!(
            s.add_command(&id, "x", "y", Some(r"..\escape"), 2),
            Err(ProjectError::InvalidCommand(_))
        ));
        s.remove_command(&id, &cmd).unwrap();
        assert!(matches!(
            s.remove_command(&id, &cmd),
            Err(ProjectError::UnknownCommand(_))
        ));
    }
}
