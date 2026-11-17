//! Local addresses without ports (proxy), HTTPS (local certificate authority) and
//! `*.test` names (hosts file). The core crate does the work; this module keeps the
//! listeners in step with Settings and answers the Settings screen.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use devdock_core::error::ErrorDto;
use devdock_core::hosts;
use devdock_core::proxy::tls::{Authority, CA_CERT_FILE};
use devdock_core::proxy::{Proxy, Route, Router};
use devdock_core::run::serve::{self, PortMode};
use devdock_core::run::{RunInfo, Runner};
use devdock_core::state::{AppState, lock};
use serde::Serialize;
use tauri::State;

use crate::commands::Core;

type CmdResult<T> = Result<T, ErrorDto>;

/// The running listeners and the last start errors, shown in Settings.
#[derive(Default)]
pub struct ProxyHub {
    http: Option<Proxy>,
    https: Option<Proxy>,
    http_error: Option<String>,
    https_error: Option<String>,
    /// Set when a listener runs on `::1` only (IPv4 port held by another program).
    http_note: Option<String>,
    https_note: Option<String>,
    ca: Option<Arc<Authority>>,
    /// Last known: is the authority in the user's trusted roots? HTTPS addresses
    /// are only offered in the Run tab when it is.
    trusted: bool,
}

fn hub(core: &Core) -> std::sync::MutexGuard<'_, ProxyHub> {
    core.proxy.lock().unwrap_or_else(|p| p.into_inner())
}

/// Routes `shop.localhost` to the project's running server, and
/// `api.shop.localhost` to its command named `api`.
struct AppRouter {
    state: Arc<Mutex<AppState>>,
    runner: Arc<Runner>,
}

impl Router for AppRouter {
    fn route(&self, host: &str) -> Route {
        let runs = self.runner.list();
        let state = lock(&self.state);
        let auto = state.settings().auto_port;
        let found = state.projects().iter().find_map(|p| {
            let h = state.host(&p.id).ok()?;
            if host == h {
                Some((p, None))
            } else {
                host.strip_suffix(&format!(".{h}"))
                    .map(|label| (p, Some(label.to_owned())))
            }
        });
        let Some((project, label)) = found else {
            return Route::Unknown;
        };
        let mut active: Vec<&RunInfo> = runs
            .iter()
            .filter(|r| r.key.project_id == project.id && r.state.is_active())
            .filter(|r| {
                label
                    .as_deref()
                    .is_none_or(|l| serve::slug(&r.label.command_name) == l)
            })
            .collect();
        active.sort_by_key(|r| r.started_at);
        for r in active {
            let prefs = state.run_prefs(&project.id, &r.key.command_id);
            let port = if prefs.mode(auto) == PortMode::Default {
                r.port_hint
                    .or_else(|| project.ports.get(&r.key.command_id.0).copied())
            } else {
                prefs.port.or(r.port_hint)
            };
            if let Some(port) = port {
                return Route::Port(port);
            }
        }
        Route::Idle {
            project: project.name.clone(),
        }
    }
}

fn data_dir(core: &Core) -> PathBuf {
    core.saved_to
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn authority(core: &Core, hub: &mut ProxyHub) -> Result<Arc<Authority>, String> {
    if let Some(ca) = &hub.ca {
        return Ok(Arc::clone(ca));
    }
    let ca = Arc::new(Authority::load_or_create(&data_dir(core)).map_err(|e| e.to_string())?);
    hub.ca = Some(Arc::clone(&ca));
    Ok(ca)
}

/// Starts, stops or moves the listeners to match Settings. Called at startup and
/// whenever Settings change.
pub fn apply(core: &Core) {
    let (settings, wanted) = {
        let state = lock(&core.state);
        (state.settings().clone(), state.wants_proxy())
    };
    let router: Arc<dyn Router> = Arc::new(AppRouter {
        state: Arc::clone(&core.state),
        runner: Arc::clone(&core.runner),
    });
    let want_http = wanted.then_some(settings.proxy_port);
    let want_https = (wanted && settings.https).then_some(settings.https_port);

    let mut h = hub(core);
    if h.http.as_ref().map(|p| p.port) != want_http {
        h.http = None;
        h.http_error = None;
        h.http_note = None;
        if let Some(port) = want_http {
            match Proxy::start(port, Arc::clone(&router), None) {
                Ok(p) => {
                    h.http_note = p.ipv4_error.as_ref().map(|_| ipv6_only_note(core, port));
                    h.http = Some(p);
                }
                Err(e) => h.http_error = Some((port, e).into_message(core)),
            }
        }
    }
    if h.https.as_ref().map(|p| p.port) != want_https {
        h.https = None;
        h.https_error = None;
        h.https_note = None;
        if let Some(port) = want_https {
            let started = authority(core, &mut h).and_then(|ca| {
                let config = ca.server_config().map_err(|e| e.to_string())?;
                Proxy::start(port, Arc::clone(&router), Some(config))
                    .map_err(|e| (port, e).into_message(core))
            });
            match started {
                Ok(p) => {
                    h.trusted = h.ca.as_deref().is_some_and(is_trusted);
                    h.https_note = p.ipv4_error.as_ref().map(|_| ipv6_only_note(core, port));
                    h.https = Some(p);
                }
                Err(msg) => h.https_error = Some(msg),
            }
        }
    }
}

trait BindMessage {
    fn into_message(self, core: &Core) -> String;
}

impl BindMessage for (u16, io::Error) {
    /// "Port 80 is used by httpd (PID 912)..." when we can tell who holds it.
    fn into_message(self, core: &Core) -> String {
        let (port, err) = self;
        match (err.kind(), holder(core, port)) {
            (_, Some(who)) => {
                format!("Port {port} is already used by {who}. Pick another port, like 8080.")
            }
            (io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied, None) => format!(
                "Windows won't let DevDock use port {port} (it may be reserved by IIS or \
                 http.sys). Pick another port, like 8080."
            ),
            (_, None) => format!("DevDock couldn't listen on port {port}: {err}."),
        }
    }
}

/// "nginx.exe (PID 7880)" for whatever listens on `port`, if Windows tells us.
fn holder(core: &Core, port: u16) -> Option<String> {
    crate::ports::rows(core).ok().and_then(|rows| {
        rows.into_iter()
            .find(|r| r.port == port && r.run.is_none())
            .map(|r| {
                let name = r
                    .process
                    .map_or_else(|| "another program".to_owned(), |p| p.name);
                format!("{name} (PID {})", r.pid)
            })
    })
}

/// Shown when only `[::1]:port` could be used.
fn ipv6_only_note(core: &Core, port: u16) -> String {
    let who = holder(core, port).unwrap_or_else(|| "another program".to_owned());
    format!(
        "{who} also uses port {port}, so DevDock shares it over IPv6. Browsers handle this \
         on their own; nothing to do."
    )
}

/// Why the HTTP proxy is on IPv6 only, if it is.
pub fn http_note(core: &Core) -> Option<String> {
    hub(core).http_note.clone()
}

/// What the Run tab needs to build addresses: the ports that are live now.
#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct LivePorts {
    pub http: Option<u16>,
    pub https: Option<u16>,
}

/// Why the HTTP proxy isn't listening, if it failed to start.
pub fn http_error(core: &Core) -> Option<String> {
    hub(core).http_error.clone()
}

pub fn live(core: &Core) -> LivePorts {
    let h = hub(core);
    LivePorts {
        http: h.http.as_ref().map(|p| p.port),
        https: h.https.as_ref().filter(|_| h.trusted).map(|p| p.port),
    }
}

/// Whether a `*.test` host is missing from the hosts file (and so won't resolve).
pub fn hosts_missing(host: &str) -> bool {
    serve::needs_hosts_entry(host)
        && fs::read_to_string(hosts::hosts_path())
            .map(|t| !hosts::listed(&t).iter().any(|n| n == host))
            .unwrap_or(true)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaStatus {
    path: String,
    trusted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsStatus {
    path: String,
    /// `*.test` addresses projects use.
    needed: Vec<String>,
    /// Of those, the ones the hosts file doesn't map yet.
    missing: Vec<String>,
    /// DevDock's block lists names no project uses any more.
    stale: bool,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    #[serde(flatten)]
    live: LivePorts,
    http_error: Option<String>,
    https_error: Option<String>,
    http_note: Option<String>,
    https_note: Option<String>,
    /// `None` until HTTPS was turned on once (the authority is created then).
    ca: Option<CaStatus>,
    hosts: HostsStatus,
}

fn certutil() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    PathBuf::from(root).join("System32\\certutil.exe")
}

fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// Whether the current user's Root store holds this authority (by thumbprint).
fn is_trusted(ca: &Authority) -> bool {
    quiet(Command::new(certutil()).args(["-user", "-verifystore", "Root", &ca.thumbprint]))
        .output()
        .is_ok_and(|o| o.status.success())
}

fn needed_hosts(core: &Core) -> Vec<String> {
    let state = lock(&core.state);
    let mut out: Vec<String> = state
        .projects()
        .iter()
        .filter_map(|p| state.host(&p.id).ok())
        .filter(|h| serve::needs_hosts_entry(h))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn hosts_status(core: &Core) -> HostsStatus {
    let path = hosts::hosts_path();
    let needed = needed_hosts(core);
    let (missing, stale, error) = match fs::read_to_string(&path) {
        Ok(text) => {
            let listed = hosts::listed(&text);
            let missing = needed
                .iter()
                .filter(|n| !listed.contains(n))
                .cloned()
                .collect();
            let stale = hosts::managed(&text).iter().any(|n| !needed.contains(n));
            (missing, stale, None)
        }
        Err(e) => (
            needed.clone(),
            false,
            Some(format!("DevDock couldn't read the hosts file: {e}.")),
        ),
    };
    HostsStatus {
        path: path.display().to_string(),
        needed,
        missing,
        stale,
        error,
    }
}

#[tauri::command(async)]
pub fn proxy_status(core: State<'_, Core>) -> ProxyStatus {
    let ca = {
        let mut h = hub(&core);
        let exists = data_dir(&core).join(CA_CERT_FILE).is_file();
        (h.ca.is_some() || exists)
            .then(|| authority(&core, &mut h).ok())
            .flatten()
    };
    let trusted = ca.as_deref().is_some_and(is_trusted);
    let mut h = hub(&core);
    h.trusted = trusted;
    ProxyStatus {
        live: LivePorts {
            http: h.http.as_ref().map(|p| p.port),
            https: h.https.as_ref().map(|p| p.port),
        },
        http_error: h.http_error.clone(),
        https_error: h.https_error.clone(),
        http_note: h.http_note.clone(),
        https_note: h.https_note.clone(),
        ca: ca.map(|ca| CaStatus {
            path: ca.cert_path.display().to_string(),
            trusted,
        }),
        hosts: {
            drop(h);
            hosts_status(&core)
        },
    }
}

fn error(kind: &'static str, message: String, fix: Option<&str>) -> ErrorDto {
    ErrorDto {
        kind,
        message,
        fix: fix.map(str::to_owned),
    }
}

/// Adds DevDock's authority to the current user's trusted roots. Windows shows its
/// own warning dialog; declining it leaves everything as it was.
#[tauri::command(async)]
pub fn ca_trust(core: State<'_, Core>) -> CmdResult<()> {
    let ca = authority(&core, &mut hub(&core)).map_err(|m| error("caFailed", m, None))?;
    let status = quiet(
        Command::new(certutil())
            .arg("-user")
            .arg("-addstore")
            .arg("Root")
            .arg(&ca.cert_path),
    )
    .status()
    .map_err(|e| {
        error(
            "caFailed",
            format!("DevDock couldn't run certutil: {e}."),
            None,
        )
    })?;
    if !status.success() || !is_trusted(&ca) {
        return Err(error(
            "caDeclined",
            "The certificate wasn't added.".into(),
            Some("Press Trust again and choose Yes in the Windows dialog."),
        ));
    }
    hub(&core).trusted = true;
    Ok(())
}

/// Removes DevDock's authority from the current user's trusted roots.
#[tauri::command(async)]
pub fn ca_untrust(core: State<'_, Core>) -> CmdResult<()> {
    let ca = authority(&core, &mut hub(&core)).map_err(|m| error("caFailed", m, None))?;
    let _ = quiet(Command::new(certutil()).args(["-user", "-delstore", "Root", &ca.thumbprint]))
        .status();
    hub(&core).trusted = false;
    if is_trusted(&ca) {
        return Err(error(
            "caDeclined",
            "The certificate is still trusted.".into(),
            Some("Press Remove again and choose Yes in the Windows dialog."),
        ));
    }
    Ok(())
}

/// Writes DevDock's block in the hosts file for every `*.test` address. Needs
/// administrator rights: Windows asks (UAC) before the copy runs.
#[tauri::command(async)]
pub fn hosts_sync(core: State<'_, Core>) -> CmdResult<()> {
    let path = hosts::hosts_path();
    let text = fs::read(&path)
        .map_err(|e| {
            error(
                "hostsFailed",
                format!("DevDock couldn't read {}: {e}.", path.display()),
                None,
            )
        })
        .and_then(|bytes| {
            String::from_utf8(bytes).map_err(|_| {
                error(
                    "hostsFailed",
                    "The hosts file isn't plain UTF-8 text, so DevDock won't rewrite it.".into(),
                    Some("Add the lines yourself: 127.0.0.1 <name>.test"),
                )
            })
        })?;
    let merged = hosts::merge(&text, &needed_hosts(&core));
    if merged == text {
        return Ok(());
    }
    let tmp = data_dir(&core).join("hosts.devdock.tmp");
    fs::write(&tmp, &merged).map_err(|e| {
        error(
            "hostsFailed",
            format!("DevDock couldn't write {}: {e}.", tmp.display()),
            None,
        )
    })?;
    let cmd = std::env::var_os("ComSpec").map_or_else(|| PathBuf::from("cmd.exe"), PathBuf::from);
    let args = format!(
        "/d /c copy /y \"{}\" \"{}\" >nul && ipconfig /flushdns >nul",
        tmp.display(),
        path.display()
    );
    let result = run_elevated(&cmd, &args);
    let _ = fs::remove_file(&tmp);
    match result {
        Ok(0) => Ok(()),
        Ok(code) => Err(error(
            "hostsFailed",
            format!("Windows couldn't update the hosts file (code {code})."),
            Some("Security software sometimes locks it; allow the change there and try again."),
        )),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => Err(error(
            "hostsDeclined",
            "The hosts file wasn't changed.".into(),
            Some("Choose Yes in the Windows prompt to let DevDock update it."),
        )),
        Err(e) => Err(error(
            "hostsFailed",
            format!("DevDock couldn't update the hosts file: {e}."),
            None,
        )),
    }
}

#[cfg(windows)]
fn run_elevated(program: &Path, args: &str) -> io::Result<u32> {
    devdock_platform::run_elevated(program, args)
}

#[cfg(not(windows))]
fn run_elevated(_: &Path, _: &str) -> io::Result<u32> {
    Err(io::Error::other("only on Windows"))
}

#[cfg(all(test, windows))]
mod tests {
    use std::time::{Duration, Instant};

    use devdock_core::run::{CommandId, RunKey, RunLabel, SpawnSpec};

    use super::*;

    #[test]
    fn routes_projects_and_command_subdomains() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = AppState::default();
        let id = state.add(dir.path(), 1).unwrap().id.clone();
        state.rename(&id, "Shop").unwrap();
        let state = Arc::new(Mutex::new(state));
        let runner = Runner::new(devdock_platform::process_tree(), Arc::new(|_, _| {}));
        let router = AppRouter {
            state: Arc::clone(&state),
            runner: Arc::clone(&runner),
        };
        assert_eq!(router.route("nope.localhost"), Route::Unknown);
        assert_eq!(
            router.route("shop.localhost"),
            Route::Idle {
                project: "Shop".into()
            }
        );

        let key = RunKey {
            project_id: id.clone(),
            command_id: CommandId("custom:api".into()),
        };
        runner
            .start(
                key.clone(),
                RunLabel {
                    project_name: "Shop".into(),
                    command_name: "API".into(),
                },
                SpawnSpec {
                    command_line: "echo listening on port 4777 & ping -n 30 127.0.0.1 >nul".into(),
                    cwd: dir.path().to_path_buf(),
                    env: Vec::new(),
                },
            )
            .unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while runner.info(&key).and_then(|r| r.port_hint).is_none() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
        // Port from the logs, by project name and by command name.
        assert_eq!(router.route("shop.localhost"), Route::Port(4777));
        assert_eq!(router.route("api.shop.localhost"), Route::Port(4777));
        assert!(matches!(
            router.route("web.shop.localhost"),
            Route::Idle { .. }
        ));

        // A fixed port wins over what the logs said.
        lock(&state)
            .set_run_prefs(
                &id,
                &key.command_id,
                Some(PortMode::Fixed),
                Some(4999),
                None,
            )
            .unwrap();
        assert_eq!(router.route("shop.localhost"), Route::Port(4999));

        runner.stop_all(Duration::from_secs(5));
    }
}
