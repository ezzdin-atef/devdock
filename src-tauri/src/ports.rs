//! Commands for the Ports screen (M5). Probing processes takes ~100 ms, so these are
//! async commands (off the main thread).

use devdock_core::error::ErrorDto;
use devdock_core::ports::{
    ConfirmRequest, Ended, PortRow, build_rows, confirm, terminate_external,
};
use devdock_core::state::lock;
use devdock_core::time::now_ms;
use serde::Serialize;
use tauri::State;

use crate::commands::Core;

type CmdResult<T> = Result<T, ErrorDto>;

/// Listening ports with owner, process details and matched project.
pub fn rows(core: &Core) -> CmdResult<Vec<PortRow>> {
    let listeners = core.probe.listeners().map_err(|e| ErrorDto {
        kind: "portsUnavailable",
        message: format!("Windows couldn't list listening ports: {e}."),
        fix: Some("Press Refresh to try again.".into()),
    })?;
    let mut pids: Vec<u32> = listeners.iter().map(|l| l.pid).collect();
    pids.sort_unstable();
    pids.dedup();
    let procs = core.probe.processes(&pids);
    let owned = core.runner.owned_pids();
    let state = lock(&core.state);
    Ok(build_rows(&listeners, &procs, &owned, state.projects()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortsView {
    rows: Vec<PortRow>,
    refreshed_at: u64,
}

#[tauri::command(async)]
pub fn ports_snapshot(core: State<'_, Core>) -> CmdResult<PortsView> {
    Ok(PortsView {
        rows: rows(&core)?,
        refreshed_at: now_ms(),
    })
}

/// Ends a process DevDock didn't start, after the user confirmed it in the dialog.
/// `confirm` re-checks that the PID still is the process the user saw.
#[tauri::command(async)]
pub fn end_process(core: State<'_, Core>, request: ConfirmRequest) -> CmdResult<Ended> {
    let owned = core.runner.owned_pids();
    let confirmed = confirm(&*core.probe, &request, &owned)?;
    Ok(terminate_external(&*core.probe, confirmed)?)
}
