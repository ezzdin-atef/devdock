//! `#[tauri::command]` adapters. Thin on purpose: lock state, call the domain
//! function, mark the store dirty, convert the error. No behaviour lives here.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use devdock_core::error::ErrorDto;
use devdock_core::git::Depth;
use devdock_core::git_job;
use devdock_core::groups::GroupId;
use devdock_core::launcher;
use devdock_core::library::Query;
use devdock_core::project::ProjectId;
use devdock_core::saver::Saver;
use devdock_core::scan::DEFAULT_DEPTH;
use devdock_core::scan_job;
use devdock_core::state::{AppState, lock};
use devdock_core::store::ScanRoot;
use devdock_core::time::now_ms;
use devdock_core::tools;

/// Everything the commands share. Managed by Tauri for the app's lifetime.
pub struct Core {
    pub state: Arc<Mutex<AppState>>,
    pub runner: Arc<devdock_core::run::Runner>,
    /// Cached tool discovery; refreshed by "Re-check" and when a launch misses.
    pub tools: Mutex<devdock_core::tools::ToolScan>,
    pub probe: Arc<dyn devdock_core::ports::SystemProbe>,
    /// Runs started with a one-off `PORT=` override; their port isn't learned.
    pub port_overrides: Mutex<std::collections::HashSet<devdock_core::run::RunKey>>,
    pub saver: Saver,
    /// Local-address proxy listeners (HTTP/HTTPS), kept in step with Settings.
    pub proxy: Mutex<crate::proxy::ProxyHub>,
    pub saved_to: PathBuf,
    pub notice: Mutex<Option<ErrorDto>>,
    /// Command palette items, built when the palette opens.
    pub palette: Mutex<Vec<devdock_core::palette::PaletteItem>>,
    /// Cancel flag of the running Files search; a new search sets it and replaces it.
    pub search: Mutex<Arc<std::sync::atomic::AtomicBool>>,
    /// An editor in the Files tab has unsaved changes (closing asks first).
    pub unsaved: std::sync::atomic::AtomicBool,
    /// Workspaces started this session (live or finished), by id.
    pub spaces: Mutex<
        std::collections::HashMap<
            devdock_core::workspace::WorkspaceId,
            Arc<crate::workspaces::SpaceHandle>,
        >,
    >,
}

type CmdResult<T> = Result<T, ErrorDto>;

fn to_json(value: impl serde::Serialize) -> CmdResult<Value> {
    serde_json::to_value(value).map_err(|err| ErrorDto {
        kind: "unexpected",
        message: format!("DevDock couldn't prepare data for the window: {err}."),
        fix: None,
    })
}

/// Returns (and clears) the one-time startup notice, e.g. "restored from backup".
#[tauri::command]
pub fn startup_notice(core: State<'_, Core>) -> Option<ErrorDto> {
    core.notice.lock().unwrap_or_else(|p| p.into_inner()).take()
}

/// The list as the UI should show it, plus counts, scan roots and the save location.
/// Serialised while the lock is held because the view borrows from the state.
#[tauri::command]
pub fn library(core: State<'_, Core>, query: Query) -> CmdResult<Value> {
    let state = lock(&core.state);
    let mut value = to_json(state.view(&query))?;
    if let Value::Object(map) = &mut value {
        map.insert(
            "savedTo".into(),
            core.saved_to.to_string_lossy().into_owned().into(),
        );
    }
    Ok(value)
}

#[tauri::command]
pub fn add_project(core: State<'_, Core>, path: PathBuf) -> CmdResult<Value> {
    let value = {
        let mut state = lock(&core.state);
        let id = state.add(&path, now_ms())?.id.clone();
        to_json(state.view_of(state.get(&id)?))?
    };
    core.saver.mark_dirty();
    Ok(value)
}

#[tauri::command]
pub fn remove_project(core: State<'_, Core>, id: ProjectId) -> CmdResult<()> {
    // Removing a project stops what DevDock is running for it.
    let active: Vec<_> = core
        .runner
        .list()
        .into_iter()
        .filter(|r| r.key.project_id == id && r.state.is_active())
        .map(|r| r.key)
        .collect();
    for k in &active {
        let _ = core.runner.stop(k);
    }
    lock(&core.state).remove(&id)?;
    core.runner.forget_project(&id);
    core.saver.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn rename_project(core: State<'_, Core>, id: ProjectId, name: String) -> CmdResult<()> {
    lock(&core.state).rename(&id, &name)?;
    core.saver.mark_dirty();
    Ok(())
}

/// A new group (folder) for the project list. Returns its id.
#[tauri::command]
pub fn group_create(core: State<'_, Core>, name: String) -> CmdResult<GroupId> {
    let id = lock(&core.state).create_group(&name, now_ms())?;
    core.saver.mark_dirty();
    Ok(id)
}

/// Renames a group and/or changes its colour.
#[tauri::command]
pub fn group_update(
    core: State<'_, Core>,
    id: GroupId,
    name: Option<String>,
    color: Option<String>,
) -> CmdResult<()> {
    lock(&core.state).update_group(&id, name.as_deref(), color.as_deref())?;
    core.saver.mark_dirty();
    Ok(())
}

/// Moves a group up (-1) or down (+1) in the list.
#[tauri::command]
pub fn group_move(core: State<'_, Core>, id: GroupId, delta: i32) {
    lock(&core.state).move_group(&id, delta);
    core.saver.mark_dirty();
}

/// Moves a group to a position in the list (drag and drop).
#[tauri::command]
pub fn group_reorder(core: State<'_, Core>, id: GroupId, index: usize) {
    lock(&core.state).reorder_group(&id, index);
    core.saver.mark_dirty();
}

/// Deletes a group; its projects stay, ungrouped.
#[tauri::command]
pub fn group_delete(core: State<'_, Core>, id: GroupId) {
    lock(&core.state).delete_group(&id);
    core.saver.mark_dirty();
}

/// Drag and drop: puts projects before or after `anchor`. With `regroup`, they
/// also move into `group` (`null` = ungrouped).
#[tauri::command]
pub fn projects_place(
    core: State<'_, Core>,
    ids: Vec<ProjectId>,
    anchor: ProjectId,
    after: bool,
    regroup: bool,
    group: Option<GroupId>,
) -> CmdResult<()> {
    let group = regroup.then_some(group.as_ref());
    lock(&core.state).place_projects(&ids, &anchor, after, group)?;
    core.saver.mark_dirty();
    Ok(())
}

/// Puts projects in a group, or takes them out of any group (`group: null`).
#[tauri::command]
pub fn projects_set_group(
    core: State<'_, Core>,
    ids: Vec<ProjectId>,
    group: Option<GroupId>,
) -> CmdResult<()> {
    lock(&core.state).set_group(&ids, group.as_ref())?;
    core.saver.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn set_favorite(core: State<'_, Core>, id: ProjectId, favorite: bool) -> CmdResult<()> {
    lock(&core.state).set_favorite(&id, favorite)?;
    core.saver.mark_dirty();
    Ok(())
}

/// Selecting a project counts as seeing it: drops the "new from scan" dot.
/// Returns true if the mark was cleared (the list then needs a reload).
#[tauri::command]
pub fn mark_seen(core: State<'_, Core>, id: ProjectId) -> bool {
    lock(&core.state).mark_seen(&id)
}

/// Returns the project's new id (ids follow the path).
#[tauri::command]
pub fn relocate_project(
    core: State<'_, Core>,
    id: ProjectId,
    path: PathBuf,
) -> CmdResult<ProjectId> {
    let new_id = lock(&core.state).relocate(&id, &path)?;
    core.saver.mark_dirty();
    Ok(new_id)
}

/// Re-checks missing folders and project types. Called on load and on window focus.
#[tauri::command]
pub fn refresh_projects(core: State<'_, Core>) {
    if lock(&core.state).refresh() {
        core.saver.mark_dirty();
    }
}

#[tauri::command]
pub fn open_in_explorer(core: State<'_, Core>, id: ProjectId) -> CmdResult<()> {
    let folder = folder_of(&core, &id)?;
    let explorer = tools::which("explorer");
    launcher::open_in_explorer(&folder, explorer.as_deref())?;
    opened(
        &core,
        &id,
        devdock_core::activity::Activity::OpenedInExplorer { at: now_ms() },
    )
}

#[tauri::command]
pub fn open_in_editor(core: State<'_, Core>, id: ProjectId) -> CmdResult<()> {
    open_editor(&core, &id)
}

/// Shared with the workspace "after start" preset.
pub(crate) fn open_editor(core: &Core, id: &ProjectId) -> CmdResult<()> {
    let folder = folder_of(core, id)?;
    // Resolved on every click so installing the editor works without a restart.
    let editor = lock(&core.state).settings().editor.clone();
    let path = tools::which(&editor);
    launcher::open_in_editor(&folder, &editor, path.as_deref())?;
    opened(
        core,
        id,
        devdock_core::activity::Activity::OpenedInEditor {
            editor,
            at: now_ms(),
        },
    )
}

/// Copies the path out so the lock is released before launching anything.
fn folder_of(core: &Core, id: &ProjectId) -> CmdResult<PathBuf> {
    Ok(lock(&core.state).get(id)?.path.clone())
}

fn opened(core: &Core, id: &ProjectId, what: devdock_core::activity::Activity) -> CmdResult<()> {
    {
        let mut state = lock(&core.state);
        state.touch_opened(id, now_ms())?;
        state.record_activity(id, what)?;
    }
    core.saver.mark_dirty();
    Ok(())
}

/// Refreshes git state in the background and emits `git:updated` per project.
/// `ids` empty = every project. `details` = full Git-tab facts, not just status.
#[tauri::command]
pub fn git_refresh(app: AppHandle, core: State<'_, Core>, ids: Vec<ProjectId>, details: bool) {
    let state = Arc::clone(&core.state);
    let depth = if details {
        Depth::Details
    } else {
        Depth::Status
    };
    let spawned = thread::Builder::new()
        .name("devdock-git".into())
        .spawn(move || {
            git_job::refresh(&state, &ids, depth, |id| {
                let _ = app.emit("git:updated", GitUpdated { id, details });
            });
        });
    if let Err(err) = spawned {
        eprintln!("DevDock could not start a git refresh: {err}");
    }
}

#[derive(Clone, serde::Serialize)]
struct GitUpdated<'a> {
    id: &'a ProjectId,
    details: bool,
}

fn project_dir(core: &Core, id: &ProjectId) -> CmdResult<PathBuf> {
    Ok(lock(&core.state).get(id)?.path.clone())
}

fn git_failed(err: devdock_core::git::GitError) -> ErrorDto {
    err.into()
}

/// Local branches (with upstream and ahead/behind) and remote-only branches.
#[tauri::command(async)]
pub fn git_branches(
    core: State<'_, Core>,
    id: ProjectId,
) -> CmdResult<devdock_core::git::ops::Branches> {
    let dir = project_dir(&core, &id)?;
    devdock_core::git::ops::branches(&dir).map_err(git_failed)
}

/// A Git-tab action: fetch, pull, push, switch or create a branch. Returns git's
/// own output; refreshes the project's git state afterwards.
#[tauri::command(async)]
pub fn git_op(
    app: AppHandle,
    core: State<'_, Core>,
    id: ProjectId,
    op: devdock_core::git::ops::GitOp,
) -> CmdResult<devdock_core::git::ops::OpResult> {
    let dir = project_dir(&core, &id)?;
    let result = devdock_core::git::ops::run(&dir, &op).map_err(git_failed)?;
    git_refresh(app, core, vec![id], true);
    Ok(result)
}

/// Cached Git-tab facts for one project, or null before the first refresh.
#[tauri::command]
pub fn git_details(core: State<'_, Core>, id: ProjectId) -> CmdResult<Value> {
    to_json(lock(&core.state).git().details(&id))
}

#[tauri::command]
pub fn add_scan_root(app: AppHandle, core: State<'_, Core>, path: PathBuf) -> CmdResult<()> {
    let root = lock(&core.state).add_scan_root(&path, DEFAULT_DEPTH)?;
    core.saver.mark_dirty();
    start_scan(app, &core, vec![root])
}

#[tauri::command]
pub fn remove_scan_root(core: State<'_, Core>, path: PathBuf) -> CmdResult<()> {
    lock(&core.state).remove_scan_root(Path::new(&path))?;
    core.saver.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn rescan(app: AppHandle, core: State<'_, Core>) -> CmdResult<()> {
    let roots = lock(&core.state).scan_roots().to_vec();
    start_scan(app, &core, roots)
}

/// Runs the scan on a worker thread and reports through events:
/// `scan:progress` while it runs, `scan:done` with the summary at the end.
fn start_scan(app: AppHandle, core: &Core, roots: Vec<ScanRoot>) -> CmdResult<()> {
    lock(&core.state).begin_scan()?;
    let state = Arc::clone(&core.state);
    let saver = core.saver.clone();
    thread::Builder::new()
        .name("devdock-scan".into())
        .spawn(move || {
            let summary = scan_job::run(&state, &roots, now_ms(), |tick| {
                let _ = app.emit("scan:progress", tick);
            });
            lock(&state).end_scan();
            if summary.added > 0 {
                saver.mark_dirty();
            }
            let _ = app.emit("scan:done", &summary);
        })
        .map(drop)
        .map_err(|err| {
            lock(&core.state).end_scan();
            ErrorDto {
                kind: "unexpected",
                message: format!("DevDock couldn't start the scan: {err}."),
                fix: Some("Try again.".into()),
            }
        })
}
