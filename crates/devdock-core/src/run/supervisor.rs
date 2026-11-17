//! Runs and watches commands (T4.3). Per run: one thread waits for the process,
//! two read stdout and stderr into the log buffer. The UI is told about changes
//! through `notify`; it then asks for a snapshot or for new log lines.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use serde::Serialize;

use super::logs::{LogBuffer, LogsPage, Stream, clean_line};
use super::port_hint::find_port;
use super::process::{ProcessTree, SpawnSpec, TreeHandle};
use super::spec::CommandId;
use super::state::{InvalidTransition, RunState, Transition};
use crate::project::ProjectId;
use crate::time::now_ms;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunKey {
    pub project_id: ProjectId,
    pub command_id: CommandId,
}

/// Human labels captured at start, so the Running screen can show a run even if
/// the project is renamed or removed meanwhile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunLabel {
    pub project_name: String,
    pub command_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Change {
    State,
    Logs,
    /// The run printed the port it listens on (first time this run).
    Port(u16),
}

pub type Notify = Arc<dyn Fn(&RunKey, Change) + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("{0} is already running")]
    AlreadyRunning(String),
    #[error("{0} is not running")]
    NotRunning(String),
    #[error("could not start `{command}`: {source}")]
    Spawn {
        command: String,
        source: std::io::Error,
    },
    #[error("could not stop the process tree: {0}")]
    Kill(std::io::Error),
    #[error("{0} did not stop within {1} s")]
    StopTimeout(String, u64),
    #[error("{0}")]
    Invalid(#[from] InvalidTransition),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunInfo {
    #[serde(flatten)]
    pub key: RunKey,
    #[serde(flatten)]
    pub label: RunLabel,
    pub command_line: String,
    pub cwd: String,
    #[serde(flatten)]
    pub state: RunState,
    pub process_count: Option<usize>,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub port_hint: Option<u16>,
    /// For FAILED: the last stderr line (or last output line).
    pub last_error: Option<String>,
}

struct Inner {
    state: RunState,
    label: RunLabel,
    spec: SpawnSpec,
    handle: Option<Arc<dyn TreeHandle>>,
    started_at: Option<u64>,
    ended_at: Option<u64>,
    port_hint: Option<u16>,
    last_error: Option<String>,
}

struct Run {
    key: RunKey,
    inner: Mutex<Inner>,
    /// Signalled when the state stops being active.
    ended: Condvar,
    logs: Mutex<LogBuffer>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub struct Runner {
    tree: Arc<dyn ProcessTree>,
    notify: Notify,
    runs: Mutex<HashMap<RunKey, Arc<Run>>>,
}

impl Runner {
    pub fn new(tree: Arc<dyn ProcessTree>, notify: Notify) -> Arc<Self> {
        Arc::new(Self {
            tree,
            notify,
            runs: Mutex::new(HashMap::new()),
        })
    }

    fn get(&self, key: &RunKey) -> Option<Arc<Run>> {
        lock(&self.runs).get(key).cloned()
    }

    fn describe(key: &RunKey, label: Option<&RunLabel>) -> String {
        label.map_or_else(|| key.command_id.0.clone(), |l| l.command_name.clone())
    }

    /// Starts `spec` for `key`. Fails if that command is already running.
    pub fn start(&self, key: RunKey, label: RunLabel, spec: SpawnSpec) -> Result<(), RunError> {
        let run = {
            let mut runs = lock(&self.runs);
            Arc::clone(runs.entry(key.clone()).or_insert_with(|| {
                Arc::new(Run {
                    key: key.clone(),
                    inner: Mutex::new(Inner {
                        state: RunState::Idle,
                        label: label.clone(),
                        spec: spec.clone(),
                        handle: None,
                        started_at: None,
                        ended_at: None,
                        port_hint: None,
                        last_error: None,
                    }),
                    ended: Condvar::new(),
                    logs: Mutex::new(LogBuffer::default()),
                })
            }))
        };

        {
            let mut inner = lock(&run.inner);
            if inner.state.is_active() {
                return Err(RunError::AlreadyRunning(label.command_name));
            }
            inner.state = inner.state.next(Transition::Start)?;
            inner.label = label;
            inner.spec = spec.clone();
            inner.port_hint = None;
            inner.last_error = None;
            inner.ended_at = None;
        }
        self.log(
            &run,
            Stream::System,
            format!("▶ {}   ({})", spec.command_line, spec.cwd.display()),
        );
        (self.notify)(&key, Change::State);

        let spawned = match self.tree.spawn(&spec) {
            Ok(s) => s,
            Err(source) => {
                let message = source.to_string();
                {
                    let mut inner = lock(&run.inner);
                    inner.state = inner.state.next(Transition::SpawnFailed)?;
                    inner.ended_at = Some(now_ms());
                    inner.last_error = Some(message.clone());
                }
                self.log(
                    &run,
                    Stream::System,
                    format!("■ could not start: {message}"),
                );
                run.ended.notify_all();
                (self.notify)(&key, Change::State);
                return Err(RunError::Spawn {
                    command: spec.command_line,
                    source,
                });
            }
        };

        let mut child = spawned.child;
        let handle: Arc<dyn TreeHandle> = Arc::from(spawned.tree);
        {
            let mut inner = lock(&run.inner);
            inner.state = inner.state.next(Transition::Spawned { pid: spawned.pid })?;
            inner.handle = Some(Arc::clone(&handle));
            inner.started_at = Some(now_ms());
        }
        (self.notify)(&key, Change::State);

        let readers = [
            child
                .stdout
                .take()
                .map(|p| self.reader(&run, Stream::Out, p)),
            child
                .stderr
                .take()
                .map(|p| self.reader(&run, Stream::Err, p)),
        ];

        let notify = Arc::clone(&self.notify);
        let waiter_run = Arc::clone(&run);
        thread::Builder::new()
            .name("devdock-run-wait".into())
            .spawn(move || {
                let code = child.wait().ok().and_then(|s| s.code());
                // Anything the shell left behind (an orphaned `node`) dies with the tree,
                // which also closes the pipes so the readers can finish.
                let _ = handle.kill_tree();
                for r in readers.into_iter().flatten() {
                    let _ = r.join();
                }
                let summary = {
                    let mut inner = lock(&waiter_run.inner);
                    let was_stopping = matches!(inner.state, RunState::Stopping { .. });
                    if let Ok(next) = inner.state.next(Transition::ProcessExited { code }) {
                        inner.state = next;
                    }
                    inner.handle = None;
                    inner.ended_at = Some(now_ms());
                    if matches!(inner.state, RunState::Failed { .. }) {
                        inner.last_error =
                            lock(&waiter_run.logs).last_error_line().map(str::to_owned);
                    }
                    match (was_stopping, code) {
                        (true, _) => "■ stopped".to_owned(),
                        (false, Some(c)) => format!("■ exited with code {c}"),
                        (false, None) => "■ exited".to_owned(),
                    }
                };
                lock(&waiter_run.logs).push(Stream::System, summary, now_ms());
                waiter_run.ended.notify_all();
                notify(&waiter_run.key, Change::Logs);
                notify(&waiter_run.key, Change::State);
            })
            .map(drop)
            .map_err(|source| RunError::Spawn {
                command: "waiter thread".into(),
                source,
            })
    }

    fn reader(
        &self,
        run: &Arc<Run>,
        stream: Stream,
        pipe: impl Read + Send + 'static,
    ) -> thread::JoinHandle<()> {
        let run = Arc::clone(run);
        let notify = Arc::clone(&self.notify);
        thread::spawn(move || {
            let mut reader = BufReader::new(pipe);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let text = clean_line(&String::from_utf8_lossy(&buf));
                if let Some(port) = find_port(&text) {
                    let mut inner = lock(&run.inner);
                    if inner.port_hint.is_none() {
                        inner.port_hint = Some(port);
                        drop(inner);
                        notify(&run.key, Change::State);
                        notify(&run.key, Change::Port(port));
                    }
                }
                lock(&run.logs).push(stream, text, now_ms());
                notify(&run.key, Change::Logs);
            }
        })
    }

    fn log(&self, run: &Run, stream: Stream, text: String) {
        lock(&run.logs).push(stream, text, now_ms());
        (self.notify)(&run.key, Change::Logs);
    }

    /// Ends the whole process tree. Returns once the kill was sent, not when it's done.
    pub fn stop(&self, key: &RunKey) -> Result<(), RunError> {
        let run = self
            .get(key)
            .ok_or_else(|| RunError::NotRunning(key.command_id.0.clone()))?;
        let handle = {
            let mut inner = lock(&run.inner);
            if !matches!(
                inner.state,
                RunState::Running { .. } | RunState::Stopping { .. }
            ) {
                return Err(RunError::NotRunning(Self::describe(
                    key,
                    Some(&inner.label),
                )));
            }
            inner.state = inner.state.next(Transition::StopRequested)?;
            inner.handle.clone()
        };
        (self.notify)(key, Change::State);
        if let Some(h) = handle {
            h.kill_tree().map_err(RunError::Kill)?;
        }
        Ok(())
    }

    /// Waits until the run is no longer active. Returns false on timeout.
    pub fn wait_ended(&self, key: &RunKey, timeout: Duration) -> bool {
        let Some(run) = self.get(key) else {
            return true;
        };
        let inner = lock(&run.inner);
        let (inner, _) = run
            .ended
            .wait_timeout_while(inner, timeout, |i| i.state.is_active())
            .unwrap_or_else(|p| p.into_inner());
        !inner.state.is_active()
    }

    /// Stop (if running), wait for the tree to end, start again with the same spec.
    pub fn restart(&self, key: &RunKey) -> Result<(), RunError> {
        let run = self
            .get(key)
            .ok_or_else(|| RunError::NotRunning(key.command_id.0.clone()))?;
        let (label, spec, active) = {
            let inner = lock(&run.inner);
            (
                inner.label.clone(),
                inner.spec.clone(),
                inner.state.is_active(),
            )
        };
        if active {
            self.stop(key)?;
            if !self.wait_ended(key, Duration::from_secs(10)) {
                return Err(RunError::StopTimeout(label.command_name, 10));
            }
        }
        self.start(key.clone(), label, spec)
    }

    /// Stops every active run and waits (bounded) for them to end. Used on quit.
    pub fn stop_all(&self, timeout: Duration) {
        let keys: Vec<RunKey> = self
            .list()
            .into_iter()
            .filter(|r| r.state.is_active())
            .map(|r| r.key)
            .collect();
        for k in &keys {
            let _ = self.stop(k);
        }
        for k in &keys {
            self.wait_ended(k, timeout);
        }
    }

    pub fn info(&self, key: &RunKey) -> Option<RunInfo> {
        self.get(key).map(|r| Self::info_of(&r))
    }

    fn info_of(run: &Run) -> RunInfo {
        let inner = lock(&run.inner);
        RunInfo {
            key: run.key.clone(),
            label: inner.label.clone(),
            command_line: inner.spec.command_line.clone(),
            cwd: inner.spec.cwd.display().to_string(),
            state: inner.state.clone(),
            process_count: inner.handle.as_ref().and_then(|h| h.process_count()),
            started_at: inner.started_at,
            ended_at: inner.ended_at,
            port_hint: inner.port_hint,
            last_error: inner.last_error.clone(),
        }
    }

    /// Every run this session, active first, then most recently started.
    pub fn list(&self) -> Vec<RunInfo> {
        let runs: Vec<Arc<Run>> = lock(&self.runs).values().cloned().collect();
        let mut out: Vec<RunInfo> = runs.iter().map(|r| Self::info_of(r)).collect();
        out.sort_by(|a, b| {
            b.state
                .is_active()
                .cmp(&a.state.is_active())
                .then(b.started_at.cmp(&a.started_at))
        });
        out
    }

    /// Every PID inside a DevDock job right now, with the run it belongs to.
    pub fn owned_pids(&self) -> std::collections::HashMap<u32, RunKey> {
        let runs: Vec<Arc<Run>> = lock(&self.runs).values().cloned().collect();
        let mut out = std::collections::HashMap::new();
        for run in runs {
            let handle = lock(&run.inner).handle.clone();
            if let Some(h) = handle {
                for pid in h.pids() {
                    out.insert(pid, run.key.clone());
                }
            }
        }
        out
    }

    pub fn active_count(&self) -> usize {
        self.list().iter().filter(|r| r.state.is_active()).count()
    }

    pub fn logs(&self, key: &RunKey, since: u64) -> Option<LogsPage> {
        self.get(key).map(|r| lock(&r.logs).since(since))
    }

    /// Forgets finished runs of a project (when it is removed from DevDock).
    pub fn forget_project(&self, project: &ProjectId) {
        lock(&self.runs)
            .retain(|k, r| &k.project_id != project || lock(&r.inner).state.is_active());
    }
}

#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::run::process::Spawned;

    /// Test-only tree: plain `cmd /C`, killed with `taskkill /T`. The real
    /// Job Object version lives in `devdock-platform`.
    struct PlainTree;
    struct PlainHandle(u32);

    impl TreeHandle for PlainHandle {
        fn kill_tree(&self) -> std::io::Result<()> {
            Command::new("taskkill")
                .args(["/PID", &self.0.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(drop)
        }
        fn process_count(&self) -> Option<usize> {
            None
        }
    }

    impl ProcessTree for PlainTree {
        fn spawn(&self, spec: &SpawnSpec) -> std::io::Result<Spawned> {
            use std::os::windows::process::CommandExt;
            let child = Command::new("cmd")
                .raw_arg(format!("/D /S /C \"{}\"", spec.command_line))
                .current_dir(&spec.cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(0x0800_0000)
                .spawn()?;
            let pid = child.id();
            Ok(Spawned {
                pid,
                child,
                tree: Box::new(PlainHandle(pid)),
            })
        }
    }

    fn runner() -> (Arc<Runner>, Arc<AtomicUsize>) {
        let changes = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&changes);
        let r = Runner::new(
            Arc::new(PlainTree),
            Arc::new(move |_, _| {
                c.fetch_add(1, Ordering::SeqCst);
            }),
        );
        (r, changes)
    }

    fn key(cmd: &str) -> RunKey {
        RunKey {
            project_id: ProjectId("p".into()),
            command_id: CommandId(cmd.into()),
        }
    }

    fn label() -> RunLabel {
        RunLabel {
            project_name: "demo".into(),
            command_name: "dev".into(),
        }
    }

    fn spec(line: &str) -> SpawnSpec {
        SpawnSpec {
            command_line: line.into(),
            cwd: std::env::temp_dir(),
            env: vec![],
        }
    }

    fn texts(r: &Runner, k: &RunKey) -> Vec<(Stream, String)> {
        r.logs(k, 0)
            .unwrap()
            .lines
            .into_iter()
            .map(|l| (l.stream, l.text))
            .collect()
    }

    #[test]
    fn captures_both_streams_and_exit_zero() {
        let (r, changes) = runner();
        let k = key("ok");
        r.start(
            k.clone(),
            label(),
            spec("echo hello& echo oops 1>&2& exit 0"),
        )
        .unwrap();
        assert!(r.wait_ended(&k, Duration::from_secs(10)));
        let info = r.info(&k).unwrap();
        assert_eq!(info.state, RunState::Exited { code: 0 });
        let logs = texts(&r, &k);
        assert!(logs.contains(&(Stream::Out, "hello".into())), "{logs:?}");
        assert!(
            logs.contains(&(Stream::Err, "oops ".into()))
                || logs.contains(&(Stream::Err, "oops".into())),
            "{logs:?}"
        );
        assert!(logs.last().unwrap().1.contains("exited with code 0"));
        assert!(changes.load(Ordering::SeqCst) >= 4);
    }

    #[test]
    fn non_zero_exit_fails_with_last_error() {
        let (r, _) = runner();
        let k = key("bad");
        r.start(k.clone(), label(), spec("echo Error: boom 1>&2& exit 3"))
            .unwrap();
        assert!(r.wait_ended(&k, Duration::from_secs(10)));
        let info = r.info(&k).unwrap();
        assert_eq!(info.state, RunState::Failed { code: Some(3) });
        assert!(info.last_error.unwrap().starts_with("Error: boom"));
    }

    #[test]
    fn stop_ends_a_long_process_as_stopped() {
        let (r, _) = runner();
        let k = key("long");
        r.start(k.clone(), label(), spec("ping -n 30 127.0.0.1"))
            .unwrap();
        assert!(matches!(
            r.info(&k).unwrap().state,
            RunState::Running { .. }
        ));
        assert!(matches!(
            r.start(k.clone(), label(), spec("x")),
            Err(RunError::AlreadyRunning(_))
        ));
        r.stop(&k).unwrap();
        assert!(r.wait_ended(&k, Duration::from_secs(10)));
        assert_eq!(r.info(&k).unwrap().state, RunState::Stopped);
        assert_eq!(r.active_count(), 0);
    }

    #[test]
    fn restart_runs_again_with_a_new_pid() {
        let (r, _) = runner();
        let k = key("again");
        r.start(k.clone(), label(), spec("ping -n 30 127.0.0.1"))
            .unwrap();
        let first = r.info(&k).unwrap().state.pid();
        r.restart(&k).unwrap();
        let second = r.info(&k).unwrap().state.pid();
        assert!(second.is_some() && second != first);
        r.stop_all(Duration::from_secs(10));
        assert_eq!(r.active_count(), 0);
    }

    #[test]
    fn port_hint_comes_from_output() {
        let (r, _) = runner();
        let k = key("port");
        r.start(
            k.clone(),
            label(),
            spec("echo   - Local: http://localhost:4321"),
        )
        .unwrap();
        assert!(r.wait_ended(&k, Duration::from_secs(10)));
        assert_eq!(r.info(&k).unwrap().port_hint, Some(4321));
    }

    #[test]
    fn missing_folder_fails_to_start() {
        let (r, _) = runner();
        let k = key("nowhere");
        let mut s = spec("echo hi");
        s.cwd = "Z:\\no\\such\\folder".into();
        assert!(matches!(
            r.start(k.clone(), label(), s),
            Err(RunError::Spawn { .. })
        ));
        assert_eq!(r.info(&k).unwrap().state, RunState::Failed { code: None });
    }
}
