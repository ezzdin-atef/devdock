//! Commands for workspaces (M7). Each started workspace gets a thread that owns its
//! orchestrator, ticks it every 250 ms and emits `workspace:changed` when anything
//! moves. Health checks can block for seconds, so the thread never shares the
//! orchestrator: it publishes a snapshot for readers and takes Stop as a flag.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use devdock_core::error::ErrorDto;
use devdock_core::git;
use devdock_core::project::ProjectId;
use devdock_core::run::RunInfo;
use devdock_core::state::lock;
use devdock_core::time::now_ms;
use devdock_core::workspace::driver::{RunnerDriver, run_key};
use devdock_core::workspace::logs::{MergedPage, merge};
use devdock_core::workspace::{
    AfterStart, Orchestrator, OrchestratorView, Phase, RestartPolicy, Service, Timing, Workspace,
    WorkspaceId,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::Core;

type CmdResult<T> = Result<T, ErrorDto>;

const TICK: Duration = Duration::from_millis(250);

/// The commands' side of a running workspace. The thread writes `view` after every
/// tick; commands only read it, and ask for a stop through `stop`.
pub struct SpaceHandle {
    view: Mutex<OrchestratorView>,
    stop: AtomicBool,
    settled: AtomicBool,
}

impl SpaceHandle {
    /// Starting, running or stopping.
    pub fn is_live(&self) -> bool {
        !self.settled.load(Ordering::SeqCst)
    }
}

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn invalid(message: String) -> ErrorDto {
    ErrorDto {
        kind: "workspaceInvalid",
        message,
        fix: None,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceExtra {
    project_name: String,
    /// What actually runs (command line or compose call), for the card.
    detail: String,
    run: Option<RunInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    #[serde(flatten)]
    workspace: Workspace,
    live: Option<OrchestratorView>,
    /// Service id -> labels and its latest run.
    extra: HashMap<String, ServiceExtra>,
}

fn view_of(core: &Core, ws: &Workspace) -> WorkspaceView {
    let live = guard(&core.spaces)
        .get(&ws.id)
        .map(|h| guard(&h.view).clone());
    let state = lock(&core.state);
    let extra = ws
        .services
        .iter()
        .map(|s| {
            let project = state.get(s.source.project_id()).ok();
            let detail = match &s.source {
                devdock_core::workspace::ServiceSource::Command {
                    command_id,
                    worktree,
                    ..
                } => {
                    let line = state
                        .commands(s.source.project_id())
                        .ok()
                        .and_then(|d| d.commands.into_iter().find(|c| &c.id == command_id))
                        .map_or_else(|| format!("{} (missing)", command_id.0), |c| c.command_line);
                    match worktree {
                        Some(w) => format!("{line}  @ {}", w.display()),
                        None => line,
                    }
                }
                devdock_core::workspace::ServiceSource::Compose { file, service, .. } => {
                    devdock_core::workspace::driver::compose_up_line(file, service)
                }
            };
            (
                s.id.clone(),
                ServiceExtra {
                    project_name: project
                        .map_or_else(|| "removed project".into(), |p| p.name.clone()),
                    detail,
                    run: core.runner.info(&run_key(s)),
                },
            )
        })
        .collect();
    WorkspaceView {
        workspace: ws.clone(),
        live,
        extra,
    }
}

#[tauri::command(async)]
pub fn workspaces_list(core: State<'_, Core>) -> Vec<WorkspaceView> {
    let list = lock(&core.state).workspaces().to_vec();
    list.iter().map(|w| view_of(&core, w)).collect()
}

#[tauri::command]
pub fn workspace_create(core: State<'_, Core>, name: String) -> CmdResult<WorkspaceId> {
    let id = lock(&core.state).create_workspace(&name, now_ms())?;
    core.saver.mark_dirty();
    Ok(id)
}

#[tauri::command]
pub fn workspace_rename(core: State<'_, Core>, id: WorkspaceId, name: String) -> CmdResult<()> {
    lock(&core.state).rename_workspace(&id, &name)?;
    core.saver.mark_dirty();
    Ok(())
}

fn is_live(core: &Core, id: &WorkspaceId) -> bool {
    guard(&core.spaces)
        .get(id)
        .is_some_and(|h| !h.settled.load(Ordering::SeqCst))
}

#[tauri::command]
pub fn workspace_delete(core: State<'_, Core>, id: WorkspaceId) -> CmdResult<()> {
    if is_live(&core, &id) {
        return Err(invalid("Stop the workspace before deleting it.".into()));
    }
    lock(&core.state).delete_workspace(&id);
    guard(&core.spaces).remove(&id);
    core.saver.mark_dirty();
    Ok(())
}

/// Git worktree folders of a project, for the "run in worktree" select (T7.7).
#[tauri::command(async)]
pub fn project_worktrees(core: State<'_, Core>, project_id: ProjectId) -> CmdResult<Vec<PathBuf>> {
    let path = lock(&core.state).get(&project_id)?.path.clone();
    Ok(git::repo_info(&path)
        .map(|i| {
            i.worktrees
                .into_iter()
                .filter(|w| !w.bare)
                .map(|w| w.path)
                .collect()
        })
        .unwrap_or_default())
}

#[tauri::command(async)]
pub fn workspace_upsert_service(
    core: State<'_, Core>,
    id: WorkspaceId,
    service: Service,
) -> CmdResult<()> {
    if is_live(&core, &id) {
        return Err(invalid(
            "Stop the workspace before changing its services.".into(),
        ));
    }
    let project_path = lock(&core.state)
        .get(service.source.project_id())?
        .path
        .clone();
    let worktrees: Vec<PathBuf> = git::repo_info(&project_path)
        .map(|i| i.worktrees.into_iter().map(|w| w.path).collect())
        .unwrap_or_default();
    {
        let mut state = lock(&core.state);
        state.validate_service(&service, &worktrees)?;
        state.upsert_service(&id, service)?;
    }
    core.saver.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn workspace_remove_service(
    core: State<'_, Core>,
    id: WorkspaceId,
    service_id: String,
) -> CmdResult<()> {
    if is_live(&core, &id) {
        return Err(invalid(
            "Stop the workspace before changing its services.".into(),
        ));
    }
    lock(&core.state).remove_service(&id, &service_id)?;
    core.saver.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn workspace_set_rules(
    core: State<'_, Core>,
    id: WorkspaceId,
    restart: RestartPolicy,
    after_start: AfterStart,
) -> CmdResult<()> {
    lock(&core.state).set_workspace_rules(&id, restart, after_start)?;
    core.saver.mark_dirty();
    Ok(())
}

/// Start all (T7.4): builds the orchestrator and ticks it on its own thread.
#[tauri::command]
pub fn workspace_start(app: AppHandle, core: State<'_, Core>, id: WorkspaceId) -> CmdResult<()> {
    if is_live(&core, &id) {
        return Ok(()); // already starting or running
    }
    let ws = lock(&core.state).workspace(&id)?.clone();
    if ws.services.is_empty() {
        return Err(invalid("Add a service first.".into()));
    }
    let mut orch = Orchestrator::new(&ws, Timing::default()).map_err(|e| invalid(e.to_string()))?;
    orch.start();
    let handle = Arc::new(SpaceHandle {
        view: Mutex::new(orch.view()),
        stop: AtomicBool::new(false),
        settled: AtomicBool::new(false),
    });
    guard(&core.spaces).insert(id.clone(), Arc::clone(&handle));

    let driver = RunnerDriver {
        runner: Arc::clone(&core.runner),
        state: Arc::clone(&core.state),
        workspace_name: ws.name.clone(),
    };
    let after = ws.after_start.clone();
    thread::Builder::new()
        .name("devdock-workspace".into())
        .spawn(move || {
            let mut orch = orch; // owned by this thread alone
            let mut preset_done = false;
            let mut stop_sent = false;
            loop {
                if !stop_sent && handle.stop.load(Ordering::SeqCst) {
                    stop_sent = true;
                    orch.request_stop();
                }
                let changed = orch.tick(now_ms(), &driver);
                let phase = orch.phase.clone();
                let settled = orch.is_settled();
                *guard(&handle.view) = orch.view();
                handle.settled.store(settled, Ordering::SeqCst);
                if changed {
                    let _ = app.emit("workspace:changed", &id);
                }
                if phase == Phase::Running && !preset_done {
                    preset_done = true;
                    run_after_start(&app, &after);
                }
                if settled {
                    let _ = app.emit("workspace:changed", &id);
                    break;
                }
                thread::sleep(TICK);
            }
        })
        .map(drop)
        .map_err(|e| invalid(format!("DevDock couldn't start the workspace thread: {e}")))
}

/// "Open VS Code + agent after start" (T7.8). Failures surface as an event; the
/// workspace itself keeps running.
fn run_after_start(app: &AppHandle, after: &AfterStart) {
    let Some(project) = &after.project_id else {
        return;
    };
    let core = app.state::<Core>();
    if after.open_editor
        && let Err(e) = crate::commands::open_editor(&core, project)
    {
        let _ = app.emit("workspace:preset-error", e);
    }
    if let Some(agent) = after.agent
        && let Err(e) = crate::agents::launch(&core, project, Some(agent), after.terminal, None)
    {
        let _ = app.emit("workspace:preset-error", e);
    }
}

/// Stop all, in reverse order (the ticking thread carries it out).
#[tauri::command]
pub fn workspace_stop(core: State<'_, Core>, id: WorkspaceId) {
    if let Some(h) = guard(&core.spaces).get(&id) {
        h.stop.store(true, Ordering::SeqCst);
    }
}

/// Combined logs (T7.5): new lines of every service since each cursor.
#[tauri::command]
pub fn workspace_logs(
    core: State<'_, Core>,
    id: WorkspaceId,
    cursors: HashMap<String, u64>,
) -> CmdResult<MergedPage> {
    let ws = lock(&core.state).workspace(&id)?.clone();
    let pages = ws
        .services
        .iter()
        .filter_map(|s| {
            let since = cursors.get(&s.id).copied().unwrap_or(0);
            core.runner
                .logs(&run_key(s), since)
                .map(|p| (s.id.clone(), p))
        })
        .collect();
    Ok(merge(pages))
}
