//! Command palette commands (T8.2). Items are built once when the palette opens
//! (that reads package.json files and the port table); each keystroke then only
//! re-runs the fuzzy match over the cached list.

use devdock_core::agents::AGENTS;
use devdock_core::palette::{
    ActionsSource, AgentsSource, CommandsSource, PaletteItem, PaletteSource, PortsSource,
    ProjectsSource, ResultGroup, WorkspacesSource, collect, search,
};
use devdock_core::project::Project;
use devdock_core::run::RunCommand;
use devdock_core::state::lock;
use devdock_core::workspace::WorkspaceId;
use tauri::State;

use crate::commands::Core;

const PER_GROUP: usize = 6;

/// Builds the item list and returns the results for an empty query.
#[tauri::command(async)]
pub fn palette_open(core: State<'_, Core>) -> Vec<ResultGroup> {
    let (projects, entries, workspaces) = {
        let state = lock(&core.state);
        let projects: Vec<Project> = state.projects().to_vec();
        let entries: Vec<(Project, Vec<RunCommand>)> = projects
            .iter()
            .filter(|p| p.path.is_dir())
            .map(|p| {
                let cmds = state
                    .commands(&p.id)
                    .map(|d| d.commands)
                    .unwrap_or_default();
                (p.clone(), cmds)
            })
            .collect();
        (projects, entries, state.workspaces().to_vec())
    };
    let live: Vec<WorkspaceId> = core
        .spaces
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .filter(|(_, h)| h.is_live())
        .map(|(id, _)| id.clone())
        .collect();
    let installed: Vec<_> = {
        let tools = core.tools.lock().unwrap_or_else(|p| p.into_inner());
        AGENTS
            .iter()
            .copied()
            .filter(|a| tools.path_of(a.command()).is_some())
            .collect()
    };
    // The port table is optional: if Windows won't list it, the palette still works.
    let rows = crate::ports::rows(&core).unwrap_or_default();

    let sources: [&dyn PaletteSource; 6] = [
        &ProjectsSource {
            projects: &projects,
        },
        &CommandsSource { entries: &entries },
        &AgentsSource {
            projects: &projects,
            agents: &installed,
        },
        &WorkspacesSource {
            workspaces: &workspaces,
            live: &live,
        },
        &PortsSource { rows: &rows },
        &ActionsSource,
    ];
    let items: Vec<PaletteItem> = collect(&sources);
    let first = search(&items, "", PER_GROUP);
    *core.palette.lock().unwrap_or_else(|p| p.into_inner()) = items;
    first
}

#[tauri::command]
pub fn palette_search(core: State<'_, Core>, query: String) -> Vec<ResultGroup> {
    let items = core.palette.lock().unwrap_or_else(|p| p.into_inner());
    search(&items, &query, PER_GROUP)
}
