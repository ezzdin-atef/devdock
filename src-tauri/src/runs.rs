//! Commands and events for running project commands (M4). Thin like `commands.rs`:
//! look up the command, hand it to the core `Runner`, map errors.
//!
//! Anything that can block (restart waits for the old tree to end) is an async
//! command: Tauri runs plain commands on the main thread, which would freeze the UI.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use devdock_core::error::ErrorDto;
use devdock_core::ports::{Conflict, check_port};
use devdock_core::project::ProjectId;
use devdock_core::run::serve::{self, PortMode};
use devdock_core::run::spec::{CommandSource, package_manager};
use devdock_core::run::{
    Change, CommandId, LogsPage, Notify, RunCommand, RunInfo, RunKey, RunLabel, SpawnSpec,
};
use devdock_core::state::lock;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::Core;

type CmdResult<T> = Result<T, ErrorDto>;

/// How often coalesced `run:changed` events go out. Log-heavy processes print
/// hundreds of lines a second; the UI only needs a few updates per second.
const EMIT_EVERY: Duration = Duration::from_millis(80);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunChanged {
    #[serde(flatten)]
    key: RunKey,
    state: bool,
    logs: bool,
}

/// Per run: the (started_at, ended_at) last written to the activity log.
type Seen = Mutex<HashMap<RunKey, (Option<u64>, Option<u64>)>>;

/// Builds the runner's `notify` callback: changes are collected per run and emitted
/// as one `run:changed` event per run every `EMIT_EVERY`.
pub fn coalescing_notify(app: AppHandle) -> Notify {
    let app_for_ports = app.clone();
    let pending: Arc<Mutex<HashMap<RunKey, (bool, bool)>>> = Arc::default();
    let flusher = Arc::clone(&pending);
    let spawned = thread::Builder::new()
        .name("devdock-run-events".into())
        .spawn(move || {
            loop {
                thread::sleep(EMIT_EVERY);
                let batch: Vec<_> = flusher
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .drain()
                    .collect();
                for (key, (state, logs)) in batch {
                    let _ = app.emit("run:changed", RunChanged { key, state, logs });
                }
            }
        });
    if let Err(err) = spawned {
        eprintln!("DevDock could not start the run event thread: {err}");
    }
    // (started_at, ended_at) last recorded per run, so each start/end is logged once.
    let seen: Seen = Seen::default();
    Arc::new(move |key: &RunKey, change: Change| {
        if change == Change::State {
            record_run_activity(&app_for_ports, key, &seen);
        }
        let mut map = pending.lock().unwrap_or_else(|p| p.into_inner());
        let entry = map.entry(key.clone()).or_default();
        match change {
            Change::State => entry.0 = true,
            Change::Logs => entry.1 = true,
            Change::Port(port) => {
                // Learned ports are saved so the next start can check for conflicts.
                if let Some(core) = app_for_ports.try_state::<Core>()
                    && !core
                        .port_overrides
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .contains(key)
                    && lock(&core.state).learn_port(&key.project_id, &key.command_id, port)
                {
                    core.saver.mark_dirty();
                }
            }
        }
    })
}

/// Adds "started" / "ended" entries to the project's activity (T8.1). Covers runs
/// from the Run tab and from workspaces alike.
fn record_run_activity(app: &AppHandle, key: &RunKey, seen: &Seen) {
    use devdock_core::activity::Activity;
    use devdock_core::run::RunState;
    let Some(core) = app.try_state::<Core>() else {
        return;
    };
    let Some(info) = core.runner.info(key) else {
        return;
    };
    let mut seen = seen.lock().unwrap_or_else(|p| p.into_inner());
    let last = seen.entry(key.clone()).or_default();
    let mut entries = Vec::new();
    if let Some(at) = info.started_at
        && last.0 != Some(at)
    {
        last.0 = Some(at);
        entries.push(Activity::Started {
            command: info.command_line.clone(),
            at,
        });
    }
    if let Some(at) = info.ended_at
        && last.1 != Some(at)
        && !info.state.is_active()
    {
        last.1 = Some(at);
        let (outcome, code) = match &info.state {
            RunState::Stopped => ("stopped", None),
            RunState::Exited { code } => ("exited", Some(*code)),
            RunState::Failed { code } => ("failed", *code),
            _ => ("ended", None),
        };
        entries.push(Activity::Ended {
            command: info.command_line.clone(),
            outcome: outcome.into(),
            code,
            at,
        });
    }
    drop(seen);
    if entries.is_empty() {
        return;
    }
    let mut state = lock(&core.state);
    for e in entries {
        let _ = state.record_activity(&key.project_id, e);
    }
    drop(state);
    core.saver.mark_dirty();
}

fn key(project_id: ProjectId, command_id: CommandId) -> RunKey {
    RunKey {
        project_id,
        command_id,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandView {
    #[serde(flatten)]
    command: RunCommand,
    /// The latest run of this command this session, if any.
    run: Option<RunInfo>,
    /// Port learned from an earlier run's output; checked before starting.
    known_port: Option<u16>,
    /// Port mode after applying the Settings default.
    port_mode: PortMode,
    /// Saved port for auto/fixed modes (auto: picked on the first run).
    port: Option<u16>,
    /// The user's port flag; `None` = DevDock decides (see `detected_args`).
    port_args: Option<String>,
    /// The flag DevDock would add for this script, if it knows the tool.
    detected_args: Option<&'static str>,
}

/// Everything the Run tab lists for a project, with each command's latest run.
#[tauri::command]
pub fn run_commands(core: State<'_, Core>, project_id: ProjectId) -> CmdResult<Value> {
    let (found, known, prefs, auto_default, host, portless) = {
        let state = lock(&core.state);
        let project = state.get(&project_id)?;
        (
            state.commands(&project_id)?,
            project.ports.clone(),
            project.run_prefs.clone(),
            state.settings().auto_port,
            state.host(&project_id)?,
            state.hides_port(&project_id),
        )
    };
    let commands: Vec<CommandView> = found
        .commands
        .into_iter()
        .map(|command| {
            let p = prefs.get(&command.id.0).cloned().unwrap_or_default();
            let mode = p.mode(auto_default);
            CommandView {
                known_port: known.get(&command.id.0).copied(),
                port_mode: mode,
                port: if mode == PortMode::Default {
                    None
                } else {
                    p.port
                },
                port_args: p.port_args,
                detected_args: match &command.source {
                    CommandSource::Script { body } => serve::detect_port_args(body),
                    _ => None,
                },
                run: core
                    .runner
                    .info(&key(project_id.clone(), command.id.clone())),
                command,
            }
        })
        .collect();
    serde_json::to_value(serde_json::json!({
        "commands": commands,
        "warning": found.warning,
        "hostsMissing": crate::proxy::hosts_missing(&host),
        "host": host,
        // Only projects that hide their port get the proxy's address.
        "proxy": if portless {
            crate::proxy::live(&core)
        } else {
            crate::proxy::LivePorts { http: None, https: None }
        },
        "portless": portless,
        "proxyNote": if portless { crate::proxy::http_note(&core) } else { None },
        "proxyError": if portless { crate::proxy::http_error(&core) } else { None },
    }))
    .map_err(|e| ErrorDto {
        kind: "unexpected",
        message: format!("DevDock couldn't prepare the command list: {e}."),
        fix: None,
    })
}

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum StartOutcome {
    Started,
    /// Something already listens on the command's port; nothing was started.
    Conflict {
        conflict: Box<Conflict>,
    },
}

/// Starts a command, unless its port is taken (T5.5). `force` skips the check;
/// `port` starts it once on that port (the "use another port" option).
///
/// Auto mode reuses the command's saved port, or picks and saves a free one, so it
/// never conflicts. Fixed mode and one-off ports are checked like learned ones.
/// Whenever DevDock chooses the port it sets `PORT` and, for tools it knows (or
/// with the user's own flag), adds the port flag to the command line.
#[tauri::command(async)]
pub fn run_start(
    core: State<'_, Core>,
    project_id: ProjectId,
    command_id: CommandId,
    force: Option<bool>,
    port: Option<u16>,
) -> CmdResult<StartOutcome> {
    let (label, command, known, prefs, auto_default, check) = {
        let state = lock(&core.state);
        let project = state.get(&project_id)?;
        let command = state
            .commands(&project_id)?
            .commands
            .into_iter()
            .find(|c| c.id == command_id)
            .ok_or_else(|| {
                ErrorDto::from(devdock_core::error::ProjectError::UnknownCommand(
                    command_id.0.clone(),
                ))
            })?;
        (
            RunLabel {
                project_name: project.name.clone(),
                command_name: command.name.clone(),
            },
            command,
            state.known_port(&project_id, &command_id),
            state.run_prefs(&project_id, &command_id),
            state.settings().auto_port,
            state.settings().check_ports,
        )
    };
    let k = key(project_id, command_id);
    // Installing packages never gets a port.
    let mode = if k.command_id.0.starts_with("deps:") {
        PortMode::Default
    } else {
        prefs.mode(auto_default)
    };
    let planned = match (port, mode) {
        (Some(p), _) => Some(p),
        (None, PortMode::Fixed) => prefs.port,
        (None, PortMode::Auto) => Some(auto_port(&core, &k, prefs.port)?),
        (None, PortMode::Default) => None,
    };
    let checked = match (port, mode) {
        (None, PortMode::Auto) => None,
        (None, PortMode::Default) => known,
        _ => planned,
    };
    if check
        && !force.unwrap_or(false)
        && let Some(p) = checked
        && let Some(conflict) = check_port(p, &crate::ports::rows(&core)?, &k)
    {
        return Ok(StartOutcome::Conflict {
            conflict: Box::new(conflict),
        });
    }
    let mut spec = SpawnSpec {
        command_line: command.command_line.clone(),
        cwd: command.cwd.clone(),
        env: Vec::new(),
    };
    if let Some(p) = planned {
        let served = serve::serve_on(&command, p, &prefs, package_manager(&command.cwd));
        spec.command_line = served.command_line;
        spec.env = served.env;
    }
    {
        let mut overrides = core
            .port_overrides
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if planned.is_some() {
            overrides.insert(k.clone());
        } else {
            overrides.remove(&k);
        }
    }
    core.runner.start(k, label, spec)?;
    Ok(StartOutcome::Started)
}

/// The saved auto port if nothing else holds it, else a newly picked one (saved).
fn auto_port(core: &Core, k: &RunKey, saved: Option<u16>) -> CmdResult<u16> {
    let rows = crate::ports::rows(core)?;
    let taken: std::collections::HashSet<u16> = rows
        .iter()
        .filter(|r| r.run.as_ref() != Some(k))
        .map(|r| r.port)
        .collect();
    if let Some(p) = saved
        && !taken.contains(&p)
    {
        return Ok(p);
    }
    let mut state = lock(&core.state);
    let reserved = state.reserved_ports((&k.project_id, &k.command_id));
    let seed = format!("{}/{}", k.project_id.0, k.command_id.0);
    let p = serve::pick_port(&seed, &taken, &reserved).ok_or_else(|| ErrorDto {
        kind: "noFreePort",
        message: "DevDock couldn't find a free port for this command.".into(),
        fix: Some("Set a fixed port in the command's port settings.".into()),
    })?;
    state.assign_port(&k.project_id, &k.command_id, p);
    drop(state);
    core.saver.mark_dirty();
    Ok(p)
}

/// Saves a command's port settings from the Run tab.
#[tauri::command]
pub fn run_set_prefs(
    core: State<'_, Core>,
    project_id: ProjectId,
    command_id: CommandId,
    mode: Option<PortMode>,
    port: Option<u16>,
    port_args: Option<String>,
) -> CmdResult<()> {
    lock(&core.state).set_run_prefs(&project_id, &command_id, mode, port, port_args.as_deref())?;
    core.saver.mark_dirty();
    Ok(())
}

/// Sets a project's address (`shop.localhost`, `shop.test`; empty = default) and
/// whether its port is hidden (`None` = follow Settings). Starts or stops the proxy
/// as needed. Returns the address now in use.
#[tauri::command]
pub fn set_project_address(
    core: State<'_, Core>,
    id: ProjectId,
    domain: String,
    hide_port: Option<bool>,
) -> CmdResult<String> {
    let host = lock(&core.state).set_address(&id, &domain, hide_port)?;
    core.saver.mark_dirty();
    crate::proxy::apply(&core);
    Ok(host)
}

/// Opens `http://<name>.localhost:<port>` (or plain localhost) in the browser.
#[tauri::command]
pub fn open_url(url: String) -> CmdResult<()> {
    if !serve::is_local_url(&url) {
        return Err(ErrorDto {
            kind: "badUrl",
            message: format!("DevDock only opens local dev addresses, not {url}."),
            fix: None,
        });
    }
    let explorer = devdock_core::tools::which("explorer");
    devdock_core::launcher::open_url(&url, explorer.as_deref())?;
    Ok(())
}

#[tauri::command]
pub fn run_stop(
    core: State<'_, Core>,
    project_id: ProjectId,
    command_id: CommandId,
) -> CmdResult<()> {
    core.runner.stop(&key(project_id, command_id))?;
    Ok(())
}

#[tauri::command(async)]
pub fn run_restart(
    core: State<'_, Core>,
    project_id: ProjectId,
    command_id: CommandId,
) -> CmdResult<()> {
    core.runner.restart(&key(project_id, command_id))?;
    Ok(())
}

#[tauri::command]
pub fn run_logs(
    core: State<'_, Core>,
    project_id: ProjectId,
    command_id: CommandId,
    since: u64,
) -> Option<LogsPage> {
    core.runner.logs(&key(project_id, command_id), since)
}

#[tauri::command]
pub fn runs_list(core: State<'_, Core>) -> Vec<RunInfo> {
    core.runner.list()
}

#[tauri::command(async)]
pub fn run_stop_all(core: State<'_, Core>) {
    core.runner.stop_all(Duration::from_secs(10));
}

#[tauri::command]
pub fn add_command(
    core: State<'_, Core>,
    project_id: ProjectId,
    name: String,
    command_line: String,
    subdir: Option<String>,
) -> CmdResult<CommandId> {
    let id = lock(&core.state).add_command(
        &project_id,
        &name,
        &command_line,
        subdir.as_deref(),
        devdock_core::time::now_ms(),
    )?;
    core.saver.mark_dirty();
    Ok(id)
}

#[tauri::command]
pub fn remove_command(
    core: State<'_, Core>,
    project_id: ProjectId,
    command_id: CommandId,
) -> CmdResult<()> {
    let k = key(project_id.clone(), command_id.clone());
    if core.runner.info(&k).is_some_and(|r| r.state.is_active()) {
        return Err(ErrorDto {
            kind: "commandRunning",
            message: "That command is running.".into(),
            fix: Some("Stop it before removing it.".into()),
        });
    }
    lock(&core.state).remove_command(&project_id, &command_id)?;
    core.saver.mark_dirty();
    Ok(())
}

/// Empties a command's log and its FAILED message.
#[tauri::command]
pub fn run_clear_logs(core: State<'_, Core>, project_id: ProjectId, command_id: CommandId) {
    core.runner.clear_logs(&key(project_id, command_id));
}

/// Forgets a finished run (state, error, log). Refuses while it runs.
#[tauri::command]
pub fn run_forget(
    core: State<'_, Core>,
    project_id: ProjectId,
    command_id: CommandId,
) -> CmdResult<()> {
    if core.runner.forget(&key(project_id, command_id)) {
        return Ok(());
    }
    Err(ErrorDto {
        kind: "commandRunning",
        message: "That command is still running.".into(),
        fix: Some("Stop it first, then clear it.".into()),
    })
}

/// Forgets every finished run this session. Returns how many were cleared.
#[tauri::command]
pub fn runs_clear_finished(core: State<'_, Core>) -> usize {
    core.runner.forget_finished()
}

/// Clears one project's recent activity and launch history.
#[tauri::command]
pub fn project_clear_history(core: State<'_, Core>, id: ProjectId) -> CmdResult<()> {
    lock(&core.state).clear_history(&id)?;
    core.saver.mark_dirty();
    Ok(())
}

/// Settings, Data: clears activity and launch history everywhere, finished runs,
/// and (when asked) the ports learned from earlier runs.
#[tauri::command]
pub fn clear_all_history(core: State<'_, Core>, learned_ports: bool) {
    {
        let mut state = lock(&core.state);
        state.clear_all_history();
        if learned_ports {
            state.clear_learned_ports();
        }
    }
    core.runner.forget_finished();
    core.saver.mark_dirty();
}

/// Called from the close dialog: stop everything DevDock started, save, exit.
#[tauri::command(async)]
pub fn quit_app(app: AppHandle) {
    let core = app.state::<Core>();
    core.runner.stop_all(Duration::from_secs(5));
    core.saver.flush();
    app.exit(0);
}
