//! Files tab commands: thin adapters over `devdock_core::files`. Paths are
//! relative to the project folder; the core checks they stay inside it.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;

use devdock_core::error::ErrorDto;
use devdock_core::files::{self, FileText, Format, Listing, search};
use devdock_core::project::ProjectId;
use devdock_core::state::lock;

use crate::commands::Core;

type CmdResult<T> = Result<T, ErrorDto>;

fn root(core: &Core, id: &ProjectId) -> CmdResult<PathBuf> {
    Ok(lock(&core.state).get(id)?.path.clone())
}

#[tauri::command(async)]
pub fn files_list(
    core: State<'_, Core>,
    id: ProjectId,
    dir: String,
    show_hidden: bool,
) -> CmdResult<Listing> {
    Ok(files::list(&root(&core, &id)?, &dir, show_hidden)?)
}

#[tauri::command(async)]
pub fn file_read(core: State<'_, Core>, id: ProjectId, path: String) -> CmdResult<FileText> {
    Ok(files::read(&root(&core, &id)?, &path)?)
}

/// Last-modified time, or null if the file is gone.
#[tauri::command(async)]
pub fn file_stat(core: State<'_, Core>, id: ProjectId, path: String) -> CmdResult<Option<u64>> {
    Ok(files::stat(&root(&core, &id)?, &path)?)
}

/// Returns the new last-modified time.
#[tauri::command(async)]
pub fn file_write(
    core: State<'_, Core>,
    id: ProjectId,
    path: String,
    text: String,
    format: Format,
    expected_mtime: u64,
    force: bool,
) -> CmdResult<u64> {
    Ok(files::write(
        &root(&core, &id)?,
        &path,
        &text,
        format,
        expected_mtime,
        force,
    )?)
}

/// One search at a time: starting a new one cancels the one before.
#[tauri::command(async)]
pub fn files_search(
    core: State<'_, Core>,
    id: ProjectId,
    query: search::SearchQuery,
) -> CmdResult<search::SearchResult> {
    let dir = root(&core, &id)?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut current = core.search.lock().unwrap_or_else(|p| p.into_inner());
        current.store(true, Ordering::Relaxed);
        *current = Arc::clone(&cancel);
    }
    Ok(search::search(&dir, &query, &cancel)?)
}

#[tauri::command]
pub fn files_search_cancel(core: State<'_, Core>) {
    core.search
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .store(true, Ordering::Relaxed);
}

/// The window reports whether an editor has unsaved changes, so closing the
/// app can ask first.
#[tauri::command]
pub fn files_set_unsaved(core: State<'_, Core>, unsaved: bool) {
    core.unsaved.store(unsaved, Ordering::Relaxed);
}
