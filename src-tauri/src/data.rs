//! Settings > data: scan depth, export and import (T8.4).

use std::path::PathBuf;

use devdock_core::error::ErrorDto;
use devdock_core::state::lock;
use devdock_core::store::{Saved, StoreError, export_file, read_file};
use serde::Serialize;
use tauri::State;

use crate::commands::Core;

type CmdResult<T> = Result<T, ErrorDto>;

#[tauri::command]
pub fn set_scan_root_depth(core: State<'_, Core>, path: PathBuf, depth: usize) -> CmdResult<()> {
    lock(&core.state).set_scan_root_depth(&path, depth)?;
    core.saver.mark_dirty();
    Ok(())
}

fn store_err(err: StoreError) -> ErrorDto {
    ErrorDto {
        kind: "dataFile",
        message: format!("DevDock {err}."),
        fix: Some("Pick a file DevDock exported, or check that the folder is writable.".into()),
    }
}

#[tauri::command(async)]
pub fn export_data(core: State<'_, Core>, path: PathBuf) -> CmdResult<()> {
    let saved = lock(&core.state).to_saved();
    export_file(&saved, &path).map_err(store_err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    projects: usize,
    scan_roots: usize,
    workspaces: usize,
    shortcuts: usize,
    current_projects: usize,
}

/// Reads and validates a file without changing anything, for the confirmation.
#[tauri::command(async)]
pub fn import_preview(core: State<'_, Core>, path: PathBuf) -> CmdResult<ImportPreview> {
    let saved: Saved = read_file(&path).map_err(store_err)?;
    Ok(ImportPreview {
        projects: saved.projects.len(),
        scan_roots: saved.scan_roots.len(),
        workspaces: saved.workspaces.len(),
        shortcuts: saved.shortcuts.len(),
        current_projects: lock(&core.state).projects().len(),
    })
}

/// Replaces DevDock's data with the file's. Refused while anything runs, because
/// runs and workspaces refer to the projects being replaced.
#[tauri::command(async)]
pub fn import_data(core: State<'_, Core>, path: PathBuf) -> CmdResult<()> {
    let busy = core.runner.active_count() > 0
        || core
            .spaces
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .any(|h| h.is_live());
    if busy {
        return Err(ErrorDto {
            kind: "importBusy",
            message: "DevDock is running processes for the current projects.".into(),
            fix: Some("Stop everything (Running screen), then import again.".into()),
        });
    }
    let saved = read_file(&path).map_err(store_err)?;
    lock(&core.state).replace_with(saved);
    core.saver.mark_dirty();
    core.saver.flush();
    Ok(())
}
