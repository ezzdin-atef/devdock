mod agents;
mod commands;
mod data;
mod files;
mod palette;
mod ports;
mod proxy;
mod runs;
mod security;
mod workspaces;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{App, Emitter, Manager, RunEvent, WindowEvent};

use commands::Core;
use devdock_core::error::ErrorDto;
use devdock_core::run::Runner;
use devdock_core::saver::Saver;
use devdock_core::state::AppState;
use devdock_core::store::Store;

const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// `%APPDATA%\DevDock`, or `DEVDOCK_DATA_DIR` when set (handy for testing with a
/// throwaway list without touching the real one).
fn data_dir(app: &App) -> tauri::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("DEVDOCK_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    Ok(app.path().config_dir()?.join("DevDock"))
}

fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new(data_dir(app)?);
    let saved_to = store.file();
    let loaded = store.load();
    let state = Arc::new(Mutex::new(AppState::from_saved(loaded.saved)));

    let handle = app.handle().clone();
    let saver = Saver::spawn(store, Arc::clone(&state), SAVE_DEBOUNCE, move |err| {
        let _ = handle.emit("store:error", ErrorDto::from(err));
    });
    // After a recovery the damaged file was moved aside: write a good one right away,
    // or every later start would recover (and warn) again.
    if loaded.warning.is_some() {
        saver.mark_dirty();
    }

    let runner = Runner::new(
        devdock_platform::process_tree(),
        runs::coalescing_notify(app.handle().clone()),
    );

    app.manage(Core {
        tools: Mutex::new(devdock_core::tools::ToolScan::detect()),
        probe: devdock_platform::system_probe(),
        port_overrides: Mutex::default(),
        spaces: Mutex::default(),
        proxy: Mutex::default(),
        palette: Mutex::default(),
        search: Mutex::default(),
        unsaved: Default::default(),
        tree: devdock_platform::process_tree(),
        security: Mutex::default(),
        installs: Mutex::default(),
        runner,
        state,
        saver,
        saved_to,
        notice: Mutex::new(loaded.warning.map(ErrorDto::store_warning)),
    });
    proxy::apply(&app.state::<Core>());
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(setup)
        // Closing the window while processes run asks first (T4.8). The dialog
        // calls `quit_app`, which stops them and exits.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let core = window.state::<Core>();
                let active = core.runner.active_count();
                let unsaved = core.unsaved.load(std::sync::atomic::Ordering::Relaxed);
                if active > 0 || unsaved {
                    api.prevent_close();
                    let _ = window.emit("app:close-requested", active);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::startup_notice,
            commands::library,
            commands::add_project,
            commands::remove_project,
            commands::rename_project,
            commands::set_favorite,
            commands::group_create,
            commands::group_update,
            commands::group_move,
            commands::group_reorder,
            commands::group_delete,
            commands::projects_set_group,
            commands::projects_place,
            commands::mark_seen,
            commands::relocate_project,
            commands::refresh_projects,
            commands::open_in_explorer,
            commands::open_in_editor,
            commands::add_scan_root,
            commands::remove_scan_root,
            commands::rescan,
            commands::git_refresh,
            commands::git_details,
            commands::git_branches,
            commands::git_op,
            runs::run_commands,
            runs::run_start,
            runs::run_set_prefs,
            runs::set_project_address,
            runs::open_url,
            proxy::proxy_status,
            proxy::ca_trust,
            proxy::ca_untrust,
            proxy::hosts_sync,
            runs::run_stop,
            runs::run_restart,
            runs::run_logs,
            runs::runs_list,
            runs::run_stop_all,
            runs::run_clear_logs,
            runs::run_forget,
            runs::runs_clear_finished,
            runs::project_clear_history,
            runs::clear_all_history,
            runs::add_command,
            runs::remove_command,
            runs::quit_app,
            ports::ports_snapshot,
            ports::end_process,
            agents::tools_scan,
            agents::tools_recheck,
            agents::agents_overview,
            agents::project_folders,
            agents::launch_agent,
            agents::set_settings,
            agents::set_shortcut,
            agents::remove_shortcut,
            agents::launch_shortcut,
            workspaces::workspaces_list,
            workspaces::workspace_create,
            workspaces::workspace_rename,
            workspaces::workspace_delete,
            workspaces::project_worktrees,
            workspaces::workspace_upsert_service,
            workspaces::workspace_remove_service,
            workspaces::workspace_set_rules,
            workspaces::workspace_start,
            workspaces::workspace_stop,
            workspaces::workspace_logs,
            palette::palette_open,
            palette::palette_search,
            data::set_scan_root_depth,
            data::export_data,
            data::import_preview,
            data::import_data,
            files::files_list,
            files::file_read,
            files::file_stat,
            files::file_write,
            files::files_search,
            files::files_search_cancel,
            files::files_set_unsaved,
            security::security_scanners,
            security::security_running,
            security::security_report,
            security::security_summaries,
            security::security_cancel,
            security::security_scan,
            security::security_export,
            security::security_open_advisory,
            security::security_open_file,
            security::security_installing,
            security::security_install,
            security::security_install_cancel,
            security::security_install_log,
            security::security_uninstall,
        ])
        .build(tauri::generate_context!());

    match app {
        // Write any pending change before the process ends.
        Ok(app) => app.run(|handle, event| {
            if let RunEvent::Exit = event {
                let core = handle.state::<Core>();
                // Job Objects end the trees anyway when DevDock exits; stopping first
                // lets each run record "stopped" and keeps the exit orderly.
                core.runner.stop_all(Duration::from_secs(3));
                core.saver.flush();
            }
        }),
        Err(err) => {
            eprintln!("DevDock failed to start: {err}");
            std::process::exit(1);
        }
    }
}
