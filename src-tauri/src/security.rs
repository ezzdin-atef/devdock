//! Security tab commands: thin adapters over `devdock_core::security`. A scan
//! runs on its own thread and reports through `security:progress` and
//! `security:done`; the finished report is saved, then read back by the window.
//! Installs from the Scanners list run the same way and report `security:install`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use devdock_core::error::ErrorDto;
use devdock_core::project::ProjectId;
use devdock_core::security::{
    self, History, Locator, Progress, Report, ScanOptions, ScanRequest, ScannerInfo, Summary,
    install,
};
use devdock_core::state::lock;

use crate::commands::Core;

type CmdResult<T> = Result<T, ErrorDto>;

/// A scan in flight: its cancel flag and where it is.
pub struct ActiveScan {
    cancel: Arc<AtomicBool>,
    progress: Option<Progress>,
    started_at: u64,
}

pub type ActiveScans = std::sync::Mutex<HashMap<ProjectId, ActiveScan>>;

/// The newest install of each scanner this session, by scanner id: running or
/// finished, with its log (so a failed install can still be read).
pub type ActiveInstalls = std::sync::Mutex<HashMap<String, InstallRecord>>;

pub struct InstallRecord {
    /// `Some` while it runs.
    cancel: Option<Arc<AtomicBool>>,
    log: InstallLog,
}

/// Log lines kept per install; older ones are dropped first.
const MAX_LOG_LINES: usize = 3000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallLog {
    pub tool: String,
    pub name: String,
    /// `running`, `done`, `failed` or `cancelled`.
    pub state: &'static str,
    pub stage: String,
    pub progress: Option<f64>,
    pub lines: Vec<String>,
    /// How many early lines were dropped to stay under the cap.
    pub dropped: usize,
    pub error: Option<String>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
}

fn data_dir(core: &Core) -> PathBuf {
    core.saved_to
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

fn history(core: &Core) -> History {
    History::new(&data_dir(core))
}

/// `%APPDATA%\DevDock\tools`: scanners installed by DevDock.
fn locator(core: &Core) -> Locator {
    Locator::new(data_dir(core).join("tools"))
}

fn installs(core: &Core) -> std::sync::MutexGuard<'_, HashMap<String, InstallRecord>> {
    core.installs.lock().unwrap_or_else(|p| p.into_inner())
}

fn scans(core: &Core) -> std::sync::MutexGuard<'_, HashMap<ProjectId, ActiveScan>> {
    core.security.lock().unwrap_or_else(|p| p.into_inner())
}

fn io_error(what: &str, err: &std::io::Error) -> ErrorDto {
    ErrorDto {
        kind: "io",
        message: format!("DevDock couldn't {what}: {err}."),
        fix: None,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Running {
    pub project_id: ProjectId,
    pub started_at: u64,
    pub progress: Option<Progress>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Done {
    project_id: ProjectId,
    summary: Option<Summary>,
    error: Option<ErrorDto>,
}

/// Which scanners are installed (DevDock's own copies, then PATH). Looked up
/// fresh, so installing one elsewhere and pressing "Check again" is enough.
#[tauri::command(async)]
pub fn security_scanners(core: State<'_, Core>) -> Vec<ScannerInfo> {
    security::detect(&locator(&core))
}

#[tauri::command]
pub fn security_running(core: State<'_, Core>) -> Vec<Running> {
    scans(&core)
        .iter()
        .map(|(id, s)| Running {
            project_id: id.clone(),
            started_at: s.started_at,
            progress: s.progress.clone(),
        })
        .collect()
}

#[tauri::command(async)]
pub fn security_report(core: State<'_, Core>, id: ProjectId) -> Option<Report> {
    history(&core).latest(&id.0)
}

#[tauri::command(async)]
pub fn security_summaries(core: State<'_, Core>) -> HashMap<String, Summary> {
    history(&core).summaries()
}

#[tauri::command]
pub fn security_cancel(core: State<'_, Core>, id: ProjectId) {
    if let Some(s) = scans(&core).get(&id) {
        s.cancel.store(true, Ordering::Relaxed);
    }
}

/// Starts a scan and returns at once. One scan per project at a time.
#[tauri::command]
pub fn security_scan(
    app: AppHandle,
    core: State<'_, Core>,
    id: ProjectId,
    options: ScanOptions,
) -> CmdResult<()> {
    let (name, dir) = {
        let state = lock(&core.state);
        let p = state.get(&id)?;
        (p.name.clone(), p.path.clone())
    };
    if !dir.is_dir() {
        return Err(ErrorDto {
            kind: "notFound",
            message: format!("The folder {} doesn't exist anymore.", dir.display()),
            fix: None,
        });
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut active = scans(&core);
        if active.contains_key(&id) {
            return Err(ErrorDto {
                kind: "busy",
                message: "A scan of this project is already running.".into(),
                fix: None,
            });
        }
        active.insert(
            id.clone(),
            ActiveScan {
                cancel: Arc::clone(&cancel),
                progress: None,
                started_at: devdock_core::time::now_ms(),
            },
        );
    }

    thread::spawn(move || {
        let core = app.state::<Core>();
        let store = history(&core);
        let previous = store.latest(&id.0);
        let locator = locator(&core);
        let req = ScanRequest {
            project_id: &id.0,
            project_name: &name,
            dir: &dir,
            options,
            previous: previous.as_ref(),
            locator: &locator,
        };
        let report = security::scan(&req, core.tree.as_ref(), &cancel, |p| {
            if let Some(s) = scans(&core).get_mut(&id) {
                s.progress = Some(p.clone());
            }
            let _ = app.emit("security:progress", p);
        });
        let saved = store.save(&report);
        scans(&core).remove(&id);
        let _ = app.emit(
            "security:done",
            Done {
                project_id: id.clone(),
                summary: saved.is_ok().then(|| report.summary()),
                error: saved.err().map(|e| io_error("save the scan report", &e)),
            },
        );
    });
    Ok(())
}

/// Writes the latest report as a self-contained HTML file.
#[tauri::command(async)]
pub fn security_export(
    app: AppHandle,
    core: State<'_, Core>,
    id: ProjectId,
    path: PathBuf,
) -> CmdResult<()> {
    let report = history(&core).latest(&id.0).ok_or_else(|| ErrorDto {
        kind: "notFound",
        message: "There's no scan to export yet. Run a scan first.".into(),
        fix: None,
    })?;
    let version = app.package_info().version.to_string();
    std::fs::write(&path, security::html::render(&report, &version))
        .map_err(|e| io_error("write the report", &e))
}

/// Opens a finding's advisory page. Only a link stored in this project's latest
/// report, and only http(s), so the window can't open arbitrary targets.
#[tauri::command(async)]
pub fn security_open_advisory(core: State<'_, Core>, id: ProjectId, key: String) -> CmdResult<()> {
    let url = history(&core)
        .latest(&id.0)
        .and_then(|r| r.findings.into_iter().find(|f| f.key == key))
        .and_then(|f| f.url)
        .filter(|u| u.starts_with("https://") || u.starts_with("http://"))
        .ok_or_else(|| ErrorDto {
            kind: "badUrl",
            message: "This finding has no advisory link.".into(),
            fix: None,
        })?;
    let explorer = devdock_core::tools::which("explorer");
    devdock_core::launcher::open_url(&url, explorer.as_deref())?;
    Ok(())
}

/// Opens an exported report in the default browser.
#[tauri::command(async)]
pub fn security_open_file(path: PathBuf) -> CmdResult<()> {
    let is_html = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));
    if !is_html || !path.is_file() {
        return Err(ErrorDto {
            kind: "notFound",
            message: format!("{} is not an exported report.", path.display()),
            fix: None,
        });
    }
    let explorer = devdock_core::tools::which("explorer");
    devdock_core::launcher::open_url(&path.to_string_lossy(), explorer.as_deref())?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallEvent {
    pub tool: String,
    /// `running`, `done`, `failed` or `cancelled`.
    pub state: &'static str,
    pub stage: String,
    /// 0 to 1 while a download runs; `None` when the step can't tell.
    pub progress: Option<f64>,
    /// A new log line, when this event carries one.
    pub line: Option<String>,
    pub error: Option<String>,
}

fn not_a_scanner(tool: &str) -> ErrorDto {
    ErrorDto {
        kind: "notFound",
        message: format!("DevDock doesn't know a scanner called {tool}."),
        fix: None,
    }
}

/// Installs in flight (so the tab shows them after switching away and back).
#[tauri::command]
pub fn security_installing(core: State<'_, Core>) -> Vec<InstallEvent> {
    installs(&core)
        .values()
        .filter(|r| r.cancel.is_some())
        .map(|r| InstallEvent {
            tool: r.log.tool.clone(),
            state: "running",
            stage: r.log.stage.clone(),
            progress: r.log.progress,
            line: None,
            error: None,
        })
        .collect()
}

/// The newest install log of a scanner this session, running or finished.
#[tauri::command]
pub fn security_install_log(core: State<'_, Core>, tool: String) -> Option<InstallLog> {
    installs(&core).get(&tool).map(|r| r.log.clone())
}

/// Records an update in the scanner's log and sends it to the window.
fn record(app: &AppHandle, tool: &str, change: impl FnOnce(&mut InstallLog) -> Option<String>) {
    let core = app.state::<Core>();
    let event = {
        let mut all = installs(&core);
        let Some(rec) = all.get_mut(tool) else {
            return;
        };
        let log = &mut rec.log;
        let line = change(log);
        if let Some(l) = &line {
            log.lines.push(l.clone());
            if log.lines.len() > MAX_LOG_LINES {
                let extra = log.lines.len() - MAX_LOG_LINES;
                log.lines.drain(..extra);
                log.dropped += extra;
            }
        }
        InstallEvent {
            tool: tool.to_owned(),
            state: log.state,
            stage: log.stage.clone(),
            progress: log.progress,
            line,
            error: log.error.clone(),
        }
    };
    let _ = app.emit("security:install", event);
}

/// Starts installing a scanner and returns at once; `security:install` reports
/// each step, log line and progress change.
#[tauri::command]
pub fn security_install(app: AppHandle, core: State<'_, Core>, tool: String) -> CmdResult<()> {
    let def = install::def(&tool).ok_or_else(|| not_a_scanner(&tool))?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut all = installs(&core);
        if all.get(&tool).is_some_and(|r| r.cancel.is_some()) {
            return Err(ErrorDto {
                kind: "busy",
                message: format!("{} is already being installed.", def.name),
                fix: None,
            });
        }
        all.insert(
            tool.clone(),
            InstallRecord {
                cancel: Some(Arc::clone(&cancel)),
                log: InstallLog {
                    tool: tool.clone(),
                    name: def.name.into(),
                    state: "running",
                    stage: "Starting".into(),
                    progress: None,
                    lines: Vec::new(),
                    dropped: 0,
                    error: None,
                    started_at: devdock_core::time::now_ms(),
                    finished_at: None,
                },
            },
        );
    }
    thread::spawn(move || {
        let core = app.state::<Core>();
        let loc = locator(&core);
        let result = install::install(def, &loc, core.tree.as_ref(), &cancel, |u| match u {
            install::Update::Stage(s) => record(&app, &tool, |log| {
                log.stage = s.to_owned();
                Some(format!("== {s}"))
            }),
            install::Update::Line(l) => record(&app, &tool, |_| Some(l.to_owned())),
            install::Update::Progress(p) => record(&app, &tool, |log| {
                log.progress = p;
                None
            }),
        });
        record(&app, &tool, |log| {
            log.finished_at = Some(devdock_core::time::now_ms());
            log.progress = None;
            match &result {
                Ok(path) => {
                    log.state = "done";
                    log.stage = "Installed".into();
                    Some(format!("== Done: {}", path.display()))
                }
                Err(e) if e == install::CANCELLED => {
                    log.state = "cancelled";
                    log.stage = "Cancelled".into();
                    Some("== Cancelled".into())
                }
                Err(e) => {
                    log.state = "failed";
                    log.stage = "Failed".into();
                    log.error = Some(e.clone());
                    Some(format!("== Failed: {e}"))
                }
            }
        });
        if let Some(r) = installs(&core).get_mut(&tool) {
            r.cancel = None;
        }
    });
    Ok(())
}

#[tauri::command]
pub fn security_install_cancel(core: State<'_, Core>, tool: String) {
    if let Some(cancel) = installs(&core).get(&tool).and_then(|r| r.cancel.as_ref()) {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// Removes a scanner DevDock installed.
#[tauri::command(async)]
pub fn security_uninstall(core: State<'_, Core>, tool: String) -> CmdResult<()> {
    let def = install::def(&tool).ok_or_else(|| not_a_scanner(&tool))?;
    install::uninstall(def, &locator(&core)).map_err(|message| ErrorDto {
        kind: "io",
        message,
        fix: None,
    })
}
