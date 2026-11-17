//! Commands for agents, terminals, settings and shortcuts (M6). Thin adapters over
//! `devdock_core::agents` (planning) and `launcher::spawn_plan` (spawning).

use std::path::PathBuf;

use devdock_core::agents::{AGENTS, AgentId, LaunchRecord, TERMINALS, TerminalKind, plan};
use devdock_core::error::ErrorDto;
use devdock_core::launcher;
use devdock_core::prefs::{EDITORS, Settings, Shortcut};
use devdock_core::project::ProjectId;
use devdock_core::scan::IGNORE;
use devdock_core::state::lock;
use devdock_core::time::now_ms;
use devdock_core::tools::ToolScan;
use serde::Serialize;
use tauri::State;

use crate::commands::Core;

type CmdResult<T> = Result<T, ErrorDto>;

fn tools(core: &Core) -> ToolScan {
    core.tools.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// The cached discovery result (T6.1).
#[tauri::command]
pub fn tools_scan(core: State<'_, Core>) -> ToolScan {
    tools(&core)
}

/// "Re-check": look on PATH again, e.g. after installing an agent.
#[tauri::command(async)]
pub fn tools_recheck(core: State<'_, Core>) -> ToolScan {
    let scan = ToolScan::detect();
    *core.tools.lock().unwrap_or_else(|p| p.into_inner()) = scan.clone();
    scan
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentView {
    id: AgentId,
    name: &'static str,
    command: &'static str,
    path: Option<PathBuf>,
    install_hint: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalView {
    kind: TerminalKind,
    name: &'static str,
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EditorView {
    command: &'static str,
    name: &'static str,
    path: Option<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShortcutView {
    #[serde(flatten)]
    shortcut: Shortcut,
    project_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsOverview {
    agents: Vec<AgentView>,
    terminals: Vec<TerminalView>,
    editors: Vec<EditorView>,
    settings: Settings,
    shortcuts: Vec<ShortcutView>,
    tools: ToolScan,
}

/// Everything the Agents tab, Agents screen and Settings screen show.
#[tauri::command]
pub fn agents_overview(core: State<'_, Core>) -> AgentsOverview {
    let scan = tools(&core);
    let state = lock(&core.state);
    AgentsOverview {
        agents: AGENTS
            .iter()
            .map(|&a| AgentView {
                id: a,
                name: a.name(),
                command: a.command(),
                path: scan.path_of(a.command()).map(Into::into),
                install_hint: a.install_hint(),
            })
            .collect(),
        terminals: TERMINALS
            .iter()
            .map(|&t| TerminalView {
                kind: t,
                name: t.name(),
                available: t.available(&scan),
            })
            .collect(),
        editors: EDITORS
            .iter()
            .map(|&(command, name)| EditorView {
                command,
                name,
                path: scan.path_of(command).map(Into::into),
            })
            .collect(),
        settings: state.settings().clone(),
        shortcuts: state
            .shortcuts()
            .iter()
            .map(|s| ShortcutView {
                project_name: state
                    .get(&s.project_id)
                    .map_or_else(|_| "removed project".into(), |p| p.name.clone()),
                shortcut: s.clone(),
            })
            .collect(),
        tools: scan,
    }
}

/// Top-level folders of a project, for the "working folder" select.
#[tauri::command]
pub fn project_folders(core: State<'_, Core>, project_id: ProjectId) -> CmdResult<Vec<String>> {
    let root = lock(&core.state).get(&project_id)?.path.clone();
    let mut folders: Vec<String> = std::fs::read_dir(&root)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| {
                    !n.starts_with('.') && !IGNORE.iter().any(|i| i.eq_ignore_ascii_case(n))
                })
                .collect()
        })
        .unwrap_or_default();
    folders.sort_by_key(|f| f.to_lowercase());
    folders.truncate(60);
    Ok(folders)
}

/// Opens a terminal in the project (optionally running an agent) and records it.
#[tauri::command]
pub fn launch_agent(
    core: State<'_, Core>,
    project_id: ProjectId,
    agent: Option<AgentId>,
    terminal: TerminalKind,
    subdir: Option<String>,
) -> CmdResult<()> {
    launch(&core, &project_id, agent, terminal, subdir)
}

pub(crate) fn launch(
    core: &Core,
    project_id: &ProjectId,
    agent: Option<AgentId>,
    terminal: TerminalKind,
    subdir: Option<String>,
) -> CmdResult<()> {
    let spec = lock(&core.state).launch_spec(project_id, agent, terminal, subdir.as_deref())?;

    let mut scan = tools(core);
    if let Some(a) = agent
        && scan.path_of(a.command()).is_none()
    {
        // Maybe it was installed since the last check.
        scan = ToolScan::detect();
        *core.tools.lock().unwrap_or_else(|p| p.into_inner()) = scan.clone();
        if scan.path_of(a.command()).is_none() {
            return Err(ErrorDto {
                kind: "agentMissing",
                message: format!("{} (`{}`) wasn't found on PATH.", a.name(), a.command()),
                fix: Some(format!(
                    "Install it, for example with `{}`, then press Re-check.",
                    a.install_hint()
                )),
            });
        }
    }

    let planned = plan(&spec, &scan)?;
    let label = agent.map_or(terminal.name(), AgentId::name);
    launcher::spawn_plan(&planned, label)?;

    lock(&core.state).record_launch(
        project_id,
        LaunchRecord {
            agent,
            terminal,
            subdir: subdir.filter(|s| !s.trim().is_empty()),
            at: now_ms(),
        },
    )?;
    {
        let mut state = lock(&core.state);
        let _ = state.touch_opened(project_id, now_ms());
        let _ = state.record_activity(
            project_id,
            devdock_core::activity::Activity::AgentLaunched {
                agent,
                terminal,
                at: now_ms(),
            },
        );
    }
    core.saver.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn set_settings(core: State<'_, Core>, settings: Settings) -> CmdResult<()> {
    lock(&core.state).set_settings(settings)?;
    core.saver.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn set_shortcut(core: State<'_, Core>, shortcut: Shortcut) -> CmdResult<()> {
    lock(&core.state).set_shortcut(shortcut)?;
    core.saver.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn remove_shortcut(core: State<'_, Core>, slot: u8) {
    lock(&core.state).remove_shortcut(slot);
    core.saver.mark_dirty();
}

/// `Ctrl Alt <slot>`: returns what was launched, for a short confirmation.
#[tauri::command]
pub fn launch_shortcut(core: State<'_, Core>, slot: u8) -> CmdResult<String> {
    let (shortcut, project_name) = {
        let state = lock(&core.state);
        let s = state.shortcut(slot).cloned().ok_or_else(|| ErrorDto {
            kind: "noShortcut",
            message: format!("Ctrl+Alt+{slot} isn't bound to anything."),
            fix: Some("Bind it from a project's Agents tab.".into()),
        })?;
        let name = state.get(&s.project_id)?.name.clone();
        (s, name)
    };
    launch(
        &core,
        &shortcut.project_id,
        shortcut.agent,
        shortcut.terminal,
        shortcut.subdir.clone(),
    )?;
    Ok(format!(
        "{} in {} · {}",
        shortcut.agent.map_or("Terminal", AgentId::name),
        shortcut.terminal.name(),
        project_name
    ))
}
