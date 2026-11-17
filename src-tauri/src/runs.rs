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
}

/// Everything the Run tab lists for a project, with each command's latest run.
#[tauri::command]
pub fn run_commands(core: State<'_, Core>, project_id: ProjectId) -> CmdResult<Value> {
    let (found, known): (_, std::collections::BTreeMap<String, u16>) = {
        let state = lock(&core.state);
        (
            state.commands(&project_id)?,
            state.get(&project_id)?.ports.clone(),
        )
    };
    let commands: Vec<CommandView> = found
        .commands
        .into_iter()
        .map(|command| CommandView {
            known_port: known.get(&command.id.0).copied(),
            run: core
                .runner
                .info(&key(project_id.clone(), command.id.clone())),
            command,
        })
        .collect();
    serde_json::to_value(serde_json::json!({ "commands": commands, "warning": found.warning }))
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

/// Starts a command, unless its known port is taken (T5.5). `force` skips the
/// check; `port` starts it with `PORT=<port>` (the "use another port" option).
#[tauri::command(async)]
pub fn run_start(
    core: State<'_, Core>,
    project_id: ProjectId,
    command_id: CommandId,
    force: Option<bool>,
    port: Option<u16>,
) -> CmdResult<StartOutcome> {
    let (label, mut spec, known) = {
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
            SpawnSpec {
                command_line: command.command_line,
                cwd: command.cwd,
                env: Vec::new(),
            },
            state.known_port(&project_id, &command_id),
        )
    };
    let k = key(project_id, command_id);
    let check = lock(&core.state).settings().check_ports;
    if check
        && !force.unwrap_or(false)
        && let Some(p) = port.or(known)
        && let Some(conflict) = check_port(p, &crate::ports::rows(&core)?, &k)
    {
        return Ok(StartOutcome::Conflict {
            conflict: Box::new(conflict),
        });
    }
    if let Some(p) = port {
        spec.env.push(("PORT".into(), p.to_string()));
    }
    {
        let mut overrides = core
            .port_overrides
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if port.is_some() {
            overrides.insert(k.clone());
        } else {
            overrides.remove(&k);
        }
    }
    core.runner.start(k, label, spec)?;
    Ok(StartOutcome::Started)
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

/// Called from the close dialog: stop everything DevDock started, save, exit.
#[tauri::command(async)]
pub fn quit_app(app: AppHandle) {
    let core = app.state::<Core>();
    core.runner.stop_all(Duration::from_secs(5));
    core.saver.flush();
    app.exit(0);
}
