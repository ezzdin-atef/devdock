//! Which scanners run for a project, how, and how their output is read.
//!
//! [`plan`] is pure (finding executables is injected), so tests can check what
//! would run without any scanner installed. [`run_step`] starts one through the
//! same process-tree spawner as the Run tab, so Cancel ends the scanner and
//! anything it started (`npm.cmd` -> `node`).
//!
//! Scanners are looked up in DevDock's own tools folder first (where the Install
//! button puts them), then on PATH.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use super::model::{Finding, ScanOptions};
use super::parse;
use crate::run::spec::{PackageManager, package_manager};
use crate::run::{ProcessTree, SpawnSpec};

/// How the Install button gets a scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// The Windows binary from the project's latest GitHub release, checked against
    /// its published SHA-256, into DevDock's tools folder.
    Release { repo: &'static str },
    /// `pip install` into a Python environment of its own in the tools folder.
    PythonVenv { package: &'static str },
    /// `cargo install`, into cargo's own bin folder.
    Cargo { krate: &'static str },
}

/// A scanner DevDock knows how to run (and install).
#[derive(Debug)]
pub struct ScannerDef {
    pub id: &'static str,
    pub name: &'static str,
    /// The executable's name, without `.exe`.
    pub program: &'static str,
    pub covers: &'static str,
    /// What to type to install it by hand.
    pub install: &'static str,
    pub url: &'static str,
    pub method: Method,
}

pub const GRYPE: ScannerDef = ScannerDef {
    id: "grype",
    name: "Grype",
    program: "grype",
    covers: "Known vulnerabilities in lockfiles of every ecosystem (npm, Cargo, Composer, pip, Go, ...)",
    install: "choco install grype   or   scoop install grype",
    url: "https://github.com/anchore/grype",
    method: Method::Release {
        repo: "anchore/grype",
    },
};

pub const TRIVY: ScannerDef = ScannerDef {
    id: "trivy",
    name: "Trivy",
    program: "trivy",
    covers: "Vulnerabilities, secrets, and misconfigurations in Dockerfiles, Compose, Kubernetes and Terraform",
    install: "choco install trivy   or   scoop install trivy",
    url: "https://github.com/aquasecurity/trivy",
    method: Method::Release {
        repo: "aquasecurity/trivy",
    },
};

pub const OSV: ScannerDef = ScannerDef {
    id: "osv-scanner",
    name: "OSV-Scanner",
    program: "osv-scanner",
    covers: "Vulnerabilities from the OSV.dev database, across every lockfile in the project",
    install: "scoop install osv-scanner",
    url: "https://github.com/google/osv-scanner",
    method: Method::Release {
        repo: "google/osv-scanner",
    },
};

pub const GITLEAKS: ScannerDef = ScannerDef {
    id: "gitleaks",
    name: "Gitleaks",
    program: "gitleaks",
    covers: "API keys, tokens and passwords committed to the code",
    install: "choco install gitleaks   or   scoop install gitleaks",
    url: "https://github.com/gitleaks/gitleaks",
    method: Method::Release {
        repo: "gitleaks/gitleaks",
    },
};

pub const SEMGREP: ScannerDef = ScannerDef {
    id: "semgrep",
    name: "Semgrep",
    program: "semgrep",
    covers: "Risky patterns in your own code: injection, unsafe eval, weak crypto, ...",
    install: "pip install semgrep   (needs Python 3.9+)",
    url: "https://github.com/semgrep/semgrep",
    method: Method::PythonVenv { package: "semgrep" },
};

pub const CARGO_AUDIT: ScannerDef = ScannerDef {
    id: "cargo-audit",
    name: "cargo audit",
    program: "cargo-audit",
    covers: "RustSec advisories for Cargo.lock",
    install: "cargo install cargo-audit --locked",
    url: "https://rustsec.org",
    method: Method::Cargo {
        krate: "cargo-audit",
    },
};

pub const ALL: &[&ScannerDef] = &[&GRYPE, &TRIVY, &OSV, &GITLEAKS, &SEMGREP, &CARGO_AUDIT];

pub fn by_id(id: &str) -> Option<&'static ScannerDef> {
    ALL.iter().copied().find(|d| d.id == id)
}

/// Where DevDock keeps the scanners it installs, and how it finds any scanner.
#[derive(Debug, Clone)]
pub struct Locator {
    pub tools_dir: PathBuf,
}

impl Locator {
    pub fn new(tools_dir: impl Into<PathBuf>) -> Self {
        Locator {
            tools_dir: tools_dir.into(),
        }
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.tools_dir.join("bin")
    }

    pub fn venv_dir(&self, def: &ScannerDef) -> PathBuf {
        self.tools_dir.join(def.id)
    }

    /// Where an install by DevDock puts this scanner.
    pub fn managed_path(&self, def: &ScannerDef) -> Option<PathBuf> {
        match def.method {
            Method::Release { .. } => Some(self.bin_dir().join(format!("{}.exe", def.program))),
            Method::PythonVenv { .. } => Some(
                self.venv_dir(def)
                    .join("Scripts")
                    .join(format!("{}.exe", def.program)),
            ),
            Method::Cargo { .. } => None,
        }
    }

    /// The scanner's executable: DevDock's copy first, then cargo's bin folder
    /// (often missing from PATH), then PATH.
    pub fn locate(&self, def: &ScannerDef) -> Option<PathBuf> {
        if let Some(p) = self.managed_path(def).filter(|p| p.is_file()) {
            return Some(p);
        }
        if matches!(def.method, Method::Cargo { .. })
            && let Some(p) = cargo_bin(def.program).filter(|p| p.is_file())
        {
            return Some(p);
        }
        crate::tools::which(def.program)
    }

    pub fn is_managed(&self, def: &ScannerDef, path: &Path) -> bool {
        self.managed_path(def).is_some_and(|m| m == path)
    }
}

/// `%CARGO_HOME%\bin\<program>.exe`, or `%USERPROFILE%\.cargo\bin\...`.
pub fn cargo_bin(program: &str) -> Option<PathBuf> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".cargo")))?;
    Some(home.join("bin").join(format!("{program}.exe")))
}

/// Kept out of the scanners' walk: installed copies of packages the lockfile
/// already lists, build output, and git's own files.
const GRYPE_EXCLUDES: &[&str] = &[
    "./**/node_modules/**",
    "./**/target/**",
    "./**/.git/**",
    "./**/dist/**",
    "./**/build/**",
];
const TRIVY_SKIP: &[&str] = &["**/node_modules", "**/target", "**/.git", "**/dist"];

/// Gitleaks' `dir` mode doesn't read `.gitignore`, so without this it walks
/// `node_modules` and `target` (slow, and full of false positives in build
/// output). Its default rules stay on. Not used when the project has its own
/// `.gitleaks.toml`.
const GITLEAKS_CONFIG: &str = r#"# Written by DevDock for one scan.
[extend]
useDefault = true

[allowlist]
description = "DevDock: dependencies and build output"
paths = [
  '''(^|[\\/])(node_modules|target|dist|build|out|\.next|\.nuxt|\.svelte-kit|\.venv|venv|vendor|\.git|coverage|bower_components)[\\/]''',
  '''\.(rmeta|rlib|pdb|exe|dll|so|dylib|wasm|map)$''',
  '''\.min\.js$''',
]
"#;

const DB_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const AUDIT_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const SCAN_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reader {
    Grype,
    Trivy,
    Osv,
    NodeAudit,
    CargoAudit,
    Gitleaks,
    Semgrep,
}

/// One scanner, ready to run or skipped with a reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub tool: String,
    pub name: String,
    pub reader: Reader,
    /// `None` means skipped; `skip` says why.
    pub command_line: Option<String>,
    pub skip: Option<String>,
    pub install: Option<String>,
    /// A note shown even when it ran ("history not scanned: not a git repo").
    pub note: Option<String>,
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
    /// Gitleaks writes its report to a file rather than stdout.
    pub report_file: Option<PathBuf>,
    /// A config file written before the run and deleted after (path, contents).
    pub config: Option<(PathBuf, String)>,
}

impl Step {
    fn new(def: &ScannerDef, reader: Reader, timeout: Duration) -> Self {
        Step {
            tool: def.id.into(),
            name: def.name.into(),
            reader,
            command_line: None,
            skip: None,
            install: None,
            note: None,
            env: Vec::new(),
            timeout,
            report_file: None,
            config: None,
        }
    }

    fn missing(mut self, def: &ScannerDef) -> Self {
        self.skip = Some(format!("{} is not installed.", def.name));
        self.install = Some(def.install.into());
        self
    }

    fn env(mut self, pairs: &[(&str, &str)]) -> Self {
        self.env = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        self
    }
}

pub fn quote(arg: &str) -> String {
    format!("\"{}\"", arg.replace('"', ""))
}

/// Yarn 2+ writes `__metadata:` at the top of `yarn.lock`; classic yarn doesn't.
fn yarn_berry(dir: &Path) -> bool {
    if dir.join(".yarnrc.yml").is_file() {
        return true;
    }
    let mut head = [0u8; 1024];
    std::fs::File::open(dir.join("yarn.lock"))
        .and_then(|mut f| f.read(&mut head))
        .map(|n| String::from_utf8_lossy(&head[..n]).contains("__metadata:"))
        .unwrap_or(false)
}

/// What a scan of `dir` will run. `locate` finds a scanner's executable;
/// `on_path` says whether a package manager is on PATH.
pub fn plan(
    dir: &Path,
    options: ScanOptions,
    report_file: &Path,
    locate: impl Fn(&ScannerDef) -> Option<PathBuf>,
    on_path: impl Fn(&str) -> bool,
) -> Vec<Step> {
    let exe = |def: &ScannerDef| locate(def).map(|p| quote(&p.to_string_lossy()));
    let mut steps = Vec::new();

    // Grype: every ecosystem's lockfiles.
    let step = Step::new(&GRYPE, Reader::Grype, DB_TIMEOUT);
    steps.push(match exe(&GRYPE) {
        Some(bin) => {
            let mut line = format!("{bin} dir:. -o json -q");
            for ex in GRYPE_EXCLUDES {
                line.push_str(" --exclude ");
                line.push_str(&quote(ex));
            }
            Step {
                command_line: Some(line),
                ..step.env(&[("GRYPE_CHECK_FOR_APP_UPDATE", "false")])
            }
        }
        None => step.missing(&GRYPE),
    });

    // Trivy: vulnerabilities, misconfigurations and secrets.
    let step = Step::new(&TRIVY, Reader::Trivy, DB_TIMEOUT);
    steps.push(match exe(&TRIVY) {
        Some(bin) => {
            let mut line =
                format!("{bin} fs --format json --quiet --scanners vuln,misconfig,secret");
            for d in TRIVY_SKIP {
                line.push_str(" --skip-dirs ");
                line.push_str(&quote(d));
            }
            line.push_str(" .");
            Step {
                command_line: Some(line),
                ..step.env(&[
                    ("TRIVY_SKIP_VERSION_CHECK", "true"),
                    ("TRIVY_DISABLE_VEX_NOTICE", "true"),
                ])
            }
        }
        None => step.missing(&TRIVY),
    });

    // OSV-Scanner: the OSV.dev database, every lockfile under the folder.
    let step = Step::new(&OSV, Reader::Osv, SCAN_TIMEOUT);
    steps.push(match exe(&OSV) {
        Some(bin) => Step {
            command_line: Some(format!("{bin} --format json -r .")),
            ..step
        },
        None => step.missing(&OSV),
    });

    // The package manager's own audit, for Node projects.
    if dir.join("package.json").is_file() {
        steps.push(node_step(dir, &on_path));
    }

    // cargo audit, for Rust projects.
    if dir.join("Cargo.lock").is_file() {
        let step = Step::new(&CARGO_AUDIT, Reader::CargoAudit, AUDIT_TIMEOUT);
        steps.push(match exe(&CARGO_AUDIT) {
            Some(bin) => Step {
                command_line: Some(format!("{bin} audit --json")),
                ..step
            },
            None => step.missing(&CARGO_AUDIT),
        });
    }

    // Gitleaks: secrets in the files (and history, if asked).
    let mut leaks = Step::new(&GITLEAKS, Reader::Gitleaks, SCAN_TIMEOUT);
    match exe(&GITLEAKS) {
        Some(bin) => {
            let is_repo = dir.join(".git").exists();
            let mode = if options.history && is_repo {
                "git"
            } else {
                "dir"
            };
            if options.history && !is_repo {
                leaks.note = Some(
                    "History was not scanned: this folder is not the root of a git repository."
                        .into(),
                );
            }
            let mut config = String::new();
            if !dir.join(".gitleaks.toml").is_file() {
                let file = report_file.with_extension("toml");
                config = format!(" --config {}", quote(&file.to_string_lossy()));
                leaks.config = Some((file, GITLEAKS_CONFIG.to_owned()));
            }
            leaks.command_line = Some(format!(
                "{bin} {mode} .{config} --report-format json --report-path {} --redact --no-banner --exit-code 0 --log-level error",
                quote(&report_file.to_string_lossy())
            ));
            leaks.report_file = Some(report_file.to_path_buf());
            steps.push(leaks);
        }
        None => steps.push(leaks.missing(&GITLEAKS)),
    }

    // Semgrep: risky patterns in the project's own code.
    let step = Step::new(&SEMGREP, Reader::Semgrep, DB_TIMEOUT);
    steps.push(match exe(&SEMGREP) {
        Some(bin) => Step {
            command_line: Some(format!(
                "{bin} scan --config p/default --json --quiet --metrics off --disable-version-check ."
            )),
            ..step.env(&[
                ("PYTHONUTF8", "1"),
                ("SEMGREP_ENABLE_VERSION_CHECK", "0"),
                ("SEMGREP_SEND_METRICS", "off"),
            ])
        },
        None => step.missing(&SEMGREP),
    });
    steps
}

fn node_step(dir: &Path, on_path: &impl Fn(&str) -> bool) -> Step {
    let pm = package_manager(dir);
    let program = pm.program();
    let mut step = Step {
        tool: program.into(),
        name: format!("{program} audit"),
        reader: Reader::NodeAudit,
        command_line: None,
        skip: None,
        install: None,
        note: None,
        env: Vec::new(),
        timeout: AUDIT_TIMEOUT,
        report_file: None,
        config: None,
    };
    if pm == PackageManager::Npm
        && !dir.join("package-lock.json").is_file()
        && !dir.join("npm-shrinkwrap.json").is_file()
    {
        step.skip = Some(
            "No lockfile yet: npm audit needs package-lock.json. Install packages first.".into(),
        );
        return step;
    }
    if !on_path(program) {
        step.skip = Some(format!("{program} is not installed or not on PATH."));
        return step;
    }
    step.command_line = Some(
        match pm {
            PackageManager::Npm => "npm audit --json",
            PackageManager::Pnpm => "pnpm audit --json",
            PackageManager::Yarn if yarn_berry(dir) => "yarn npm audit --all --recursive --json",
            PackageManager::Yarn => "yarn audit --json",
            PackageManager::Bun => "bun audit --json",
        }
        .into(),
    );
    step
}

// --- Running a process ------------------------------------------------------------

pub struct Captured {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub enum ExecError {
    Cancelled,
    TimedOut,
    Failed(String),
}

/// Reads a pipe to the end. With `lines`, also sends each line as it arrives
/// (split on `\n` and `\r`, so progress bars that redraw a line still show).
fn drain(
    pipe: Option<impl Read + Send + 'static>,
    lines: Option<mpsc::Sender<String>>,
) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut all = Vec::new();
        let Some(mut p) = pipe else {
            return String::new();
        };
        let mut pending = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = match p.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            all.extend_from_slice(&buf[..n]);
            let Some(tx) = &lines else { continue };
            for &b in &buf[..n] {
                if b == b'\n' || b == b'\r' {
                    let text = String::from_utf8_lossy(&pending).trim_end().to_owned();
                    if !text.trim().is_empty() {
                        let _ = tx.send(text);
                    }
                    pending.clear();
                } else {
                    pending.push(b);
                }
            }
        }
        if let Some(tx) = &lines {
            let text = String::from_utf8_lossy(&pending).trim_end().to_owned();
            if !text.trim().is_empty() {
                let _ = tx.send(text);
            }
        }
        String::from_utf8_lossy(&all).into_owned()
    })
}

/// Runs `line` through `cmd` in `cwd`, in its own process tree, and waits.
/// Cancel and the timeout end the whole tree.
pub fn exec(
    tree: &dyn ProcessTree,
    cwd: &Path,
    line: &str,
    env: &[(String, String)],
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<Captured, ExecError> {
    exec_watched(tree, cwd, line, env, timeout, cancel, None)
}

/// Callbacks for a watched run: each output line as it arrives, and a tick
/// about every 50 ms while it runs (to check a download's size, say).
pub struct Watch<'a> {
    pub line: &'a mut dyn FnMut(&str),
    pub tick: &'a mut dyn FnMut(),
}

/// [`exec`], reporting output lines and ticks while the process runs. The
/// callbacks run on the calling thread.
pub fn exec_watched(
    tree: &dyn ProcessTree,
    cwd: &Path,
    line: &str,
    env: &[(String, String)],
    timeout: Duration,
    cancel: &AtomicBool,
    mut watch: Option<Watch<'_>>,
) -> Result<Captured, ExecError> {
    let spec = SpawnSpec {
        command_line: line.to_owned(),
        cwd: cwd.to_path_buf(),
        env: env.to_vec(),
    };
    let mut spawned = tree
        .spawn(&spec)
        .map_err(|e| ExecError::Failed(e.to_string()))?;
    let (tx, rx) = mpsc::channel::<String>();
    let streaming = watch.is_some();
    let out = drain(spawned.child.stdout.take(), streaming.then(|| tx.clone()));
    let err = drain(spawned.child.stderr.take(), streaming.then(|| tx.clone()));
    drop(tx);
    let pump = |watch: &mut Option<Watch<'_>>| {
        if let Some(w) = watch {
            while let Ok(l) = rx.try_recv() {
                (w.line)(&l);
            }
            (w.tick)();
        }
    };
    let deadline = Instant::now() + timeout;
    let status = loop {
        pump(&mut watch);
        match spawned.child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                let stop = if cancel.load(Ordering::Relaxed) {
                    Some(ExecError::Cancelled)
                } else if Instant::now() >= deadline {
                    Some(ExecError::TimedOut)
                } else {
                    None
                };
                if let Some(stop) = stop {
                    let _ = spawned.tree.kill_tree();
                    let _ = spawned.child.wait();
                    return Err(stop);
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(ExecError::Failed(e.to_string())),
        }
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    // Lines that arrived between the last check and the exit.
    if let Some(w) = &mut watch {
        while let Ok(l) = rx.try_recv() {
            (w.line)(&l);
        }
    }
    Ok(Captured {
        code: status.code(),
        stdout,
        stderr,
    })
}

pub fn last_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).rfind(|l| !l.is_empty())
}

/// How one step ended.
#[derive(Debug)]
pub enum Outcome {
    Ran { findings: Vec<Finding> },
    Failed(String),
    Cancelled,
}

/// Runs one planned step and reads its output.
pub fn run_step(tree: &dyn ProcessTree, dir: &Path, step: &Step, cancel: &AtomicBool) -> Outcome {
    let Some(line) = &step.command_line else {
        return Outcome::Ran {
            findings: Vec::new(),
        };
    };
    if let Some(f) = &step.report_file {
        let _ = std::fs::remove_file(f);
    }
    if let Some((file, text)) = &step.config
        && let Err(e) = std::fs::write(file, text)
    {
        return Outcome::Failed(format!("Couldn't write {}'s settings: {e}", step.name));
    }
    let result = exec(tree, dir, line, &step.env, step.timeout, cancel);
    if let Some((file, _)) = &step.config {
        let _ = std::fs::remove_file(file);
    }
    let captured = match result {
        Ok(c) => c,
        Err(ExecError::Cancelled) => return Outcome::Cancelled,
        Err(ExecError::TimedOut) => {
            return Outcome::Failed(format!(
                "{} took longer than {} minutes and was stopped.",
                step.name,
                step.timeout.as_secs() / 60
            ));
        }
        Err(ExecError::Failed(msg)) => {
            return Outcome::Failed(format!("Couldn't start {}: {msg}", step.name));
        }
    };

    let output = match &step.report_file {
        Some(file) => {
            let text = std::fs::read_to_string(file).unwrap_or_default();
            let _ = std::fs::remove_file(file);
            text
        }
        None => captured.stdout.clone(),
    };
    let root = dir.to_string_lossy();
    let parsed = match step.reader {
        Reader::Grype => parse::grype(&output),
        Reader::Trivy => parse::trivy(&output),
        Reader::CargoAudit => parse::cargo_audit(&output),
        Reader::Semgrep => parse::semgrep(&output, &root),
        // 128: "no package sources found", nothing to check.
        Reader::Osv if captured.code == Some(128) && output.trim().is_empty() => Ok(Vec::new()),
        Reader::Osv => parse::osv(&output, &root),
        Reader::Gitleaks => {
            if output.trim().is_empty() && captured.code != Some(0) {
                Err(String::new())
            } else {
                parse::gitleaks(&output)
            }
        }
        Reader::NodeAudit => {
            let lock = std::fs::read_to_string(dir.join("package-lock.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok());
            parse::node_audit(&step.tool, &output, lock.as_ref())
        }
    };
    match parsed {
        Ok(findings) => Outcome::Ran { findings },
        Err(msg) => Outcome::Failed(explain_failure(step, &captured, &msg)),
    }
}

fn explain_failure(step: &Step, c: &Captured, parse_msg: &str) -> String {
    let err = c.stderr.to_lowercase();
    if step.reader == Reader::Gitleaks && err.contains("unknown command") {
        return "This Gitleaks is too old: DevDock needs version 8.19 or newer.".into();
    }
    if err.contains("not recognized as an internal or external command") {
        return format!("{} could not be started.", step.name);
    }
    if c.stdout.trim().is_empty() {
        if let Some(line) = last_line(&c.stderr) {
            return format!("{} failed: {line}", step.name);
        }
        return format!(
            "{} exited with code {} and printed nothing.",
            step.name,
            c.code.map_or_else(|| "?".into(), |x| x.to_string())
        );
    }
    if parse_msg.is_empty() {
        format!("{} failed.", step.name)
    } else {
        parse_msg.to_owned()
    }
}

// --- For the Security tab -----------------------------------------------------------

/// Each scanner, whether it was found, and whether the Install button can get it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub covers: &'static str,
    pub install: &'static str,
    pub url: &'static str,
    pub path: Option<PathBuf>,
    /// Installed by DevDock (so DevDock can remove it).
    pub managed: bool,
    /// `download`, `python` or `cargo`.
    pub method: &'static str,
    /// Where the Install button gets it from, in words.
    pub source: String,
    /// Why the Install button can't be used here, if it can't.
    pub blocked: Option<String>,
    /// The version DevDock installed, when it installed it.
    pub version: Option<String>,
}

pub fn detect(locator: &Locator) -> Vec<ScannerInfo> {
    ALL.iter()
        .map(|d| {
            let path = locator.locate(d);
            let managed = path.as_deref().is_some_and(|p| locator.is_managed(d, p));
            let (method, source) = match d.method {
                Method::Release { repo } => (
                    "download",
                    format!(
                        "Downloads the latest release from github.com/{repo} and checks its SHA-256"
                    ),
                ),
                Method::PythonVenv { package } => (
                    "python",
                    format!("Installs {package} with pip into a Python environment of its own"),
                ),
                Method::Cargo { krate } => (
                    "cargo",
                    format!("Builds {krate} with cargo install (takes a few minutes)"),
                ),
            };
            let version = managed
                .then(|| super::install::installed_version(locator, d))
                .flatten();
            ScannerInfo {
                id: d.id,
                name: d.name,
                covers: d.covers,
                install: d.install,
                url: d.url,
                path,
                managed,
                method,
                source,
                blocked: super::install::blocked(d),
                version,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(steps: &[Step]) -> Vec<(&str, bool)> {
        steps
            .iter()
            .map(|s| (s.tool.as_str(), s.command_line.is_some()))
            .collect()
    }

    fn fake(def: &ScannerDef) -> Option<PathBuf> {
        Some(PathBuf::from(format!("C:\\tools\\{}.exe", def.program)))
    }

    #[test]
    fn plans_by_project_type_and_installed_tools() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "").unwrap();
        let report = dir.path().join("r.json");

        let all = plan(dir.path(), ScanOptions::default(), &report, fake, |_| true);
        assert_eq!(
            names(&all),
            vec![
                ("grype", true),
                ("trivy", true),
                ("osv-scanner", true),
                ("pnpm", true),
                ("cargo-audit", true),
                ("gitleaks", true),
                ("semgrep", true)
            ]
        );
        let line = |i: usize| all[i].command_line.as_deref().unwrap();
        assert!(line(0).starts_with("\"C:\\tools\\grype.exe\" dir:."));
        assert!(line(0).contains("node_modules"));
        assert!(line(1).contains("--scanners vuln,misconfig,secret"));
        assert!(line(5).contains(" dir . "));
        assert!(line(6).contains("--metrics off"));

        let none = plan(
            dir.path(),
            ScanOptions::default(),
            &report,
            |_| None,
            |_| false,
        );
        assert!(none.iter().all(|s| s.command_line.is_none()));
        assert!(none[0].install.as_deref().unwrap().contains("grype"));
    }

    #[test]
    fn npm_without_lockfile_is_skipped_and_history_needs_a_repo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        let opts = ScanOptions { history: true };
        let steps = plan(dir.path(), opts, Path::new("r.json"), fake, |_| true);
        let npm = steps.iter().find(|s| s.tool == "npm").unwrap();
        assert!(npm.command_line.is_none());
        assert!(npm.skip.as_deref().unwrap().contains("package-lock.json"));
        let leaks = steps.iter().find(|s| s.tool == "gitleaks").unwrap();
        assert!(leaks.command_line.as_deref().unwrap().contains(" dir ."));
        assert!(leaks.note.is_some());
        assert!(leaks.command_line.as_deref().unwrap().contains("--config"));
        assert!(
            leaks
                .config
                .as_ref()
                .unwrap()
                .1
                .contains("useDefault = true")
        );
        // A project's own Gitleaks config wins.
        std::fs::write(dir.path().join(".gitleaks.toml"), "").unwrap();
        let steps = plan(dir.path(), opts, Path::new("r.json"), fake, |_| true);
        let leaks = steps.iter().find(|s| s.tool == "gitleaks").unwrap();
        assert!(leaks.config.is_none());
        std::fs::remove_file(dir.path().join(".gitleaks.toml")).unwrap();

        std::fs::create_dir(dir.path().join(".git")).unwrap();
        let steps = plan(dir.path(), opts, Path::new("r.json"), fake, |_| true);
        let leaks = steps.iter().find(|s| s.tool == "gitleaks").unwrap();
        assert!(leaks.command_line.as_deref().unwrap().contains(" git ."));
    }

    #[test]
    fn yarn_versions_get_their_own_audit_command() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::write(dir.path().join("yarn.lock"), "# yarn lockfile v1\n").unwrap();
        let yarn = |d: &Path| {
            plan(d, ScanOptions::default(), Path::new("r"), fake, |_| true)
                .into_iter()
                .find(|s| s.tool == "yarn")
                .and_then(|s| s.command_line)
        };
        assert_eq!(yarn(dir.path()).as_deref(), Some("yarn audit --json"));
        std::fs::write(dir.path().join("yarn.lock"), "__metadata:\n  version: 8\n").unwrap();
        assert_eq!(
            yarn(dir.path()).as_deref(),
            Some("yarn npm audit --all --recursive --json")
        );
    }

    #[test]
    fn managed_copies_win_over_path() {
        let dir = tempfile::tempdir().unwrap();
        let loc = Locator::new(dir.path());
        std::fs::create_dir_all(loc.bin_dir()).unwrap();
        let trivy = loc.bin_dir().join("trivy.exe");
        std::fs::write(&trivy, "").unwrap();
        assert_eq!(loc.locate(&TRIVY), Some(trivy.clone()));
        assert!(loc.is_managed(&TRIVY, &trivy));
        assert_eq!(
            loc.managed_path(&SEMGREP).unwrap(),
            dir.path()
                .join("semgrep")
                .join("Scripts")
                .join("semgrep.exe")
        );
        assert!(loc.managed_path(&CARGO_AUDIT).is_none());
        assert_eq!(by_id("osv-scanner").unwrap().name, "OSV-Scanner");
    }
}
