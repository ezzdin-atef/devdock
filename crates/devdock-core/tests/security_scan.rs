//! Runs a real security scan against this repository with whatever scanners are
//! installed. Ignored by default (it needs the scanners and network for their
//! databases): `cargo test -p devdock-core --test security_scan -- --ignored --nocapture`.
#![cfg(windows)]

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

use devdock_core::run::{ProcessTree, SpawnSpec, Spawned, TreeHandle};
use devdock_core::security::{self, ScanOptions, ScanRequest, ToolStatus};

struct Plain;
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

impl ProcessTree for Plain {
    fn spawn(&self, spec: &SpawnSpec) -> std::io::Result<Spawned> {
        let child = Command::new("cmd")
            .raw_arg(format!("/D /S /C \"{}\"", spec.command_line))
            .current_dir(&spec.cwd)
            .envs(spec.env.iter().map(|(k, v)| (k, v)))
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

#[test]
#[ignore = "runs the real scanners"]
fn scans_this_repository() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = std::fs::canonicalize(root).unwrap();
    let cancel = AtomicBool::new(false);
    let tools = tempfile::tempdir().unwrap();
    let locator = security::Locator::new(tools.path());
    let req = ScanRequest {
        project_id: "selftest",
        project_name: "devdock",
        dir: &root,
        options: ScanOptions::default(),
        previous: None,
        locator: &locator,
    };
    let report = security::scan(&req, &Plain, &cancel, |p| {
        eprintln!("[{}/{}] {}", p.step, p.total, p.name);
    });
    for t in &report.tools {
        eprintln!(
            "{:<12} {:?} {} findings, {} ms {:?}",
            t.name, t.status, t.findings, t.ms, t.detail
        );
    }
    for f in &report.findings {
        eprintln!(
            "{:<8} {:<22} {}@{} fix={:?} tools={:?}",
            f.severity.label(),
            f.id,
            f.package.as_deref().unwrap_or("-"),
            f.version.as_deref().unwrap_or("-"),
            f.fixed_in,
            f.tools
        );
    }
    assert!(
        report.tools.iter().all(|t| t.status != ToolStatus::Failed),
        "{:#?}",
        report.tools
    );
    let out = std::env::temp_dir().join("devdock-security-selftest.html");
    std::fs::write(&out, security::html::render(&report, "test")).unwrap();
    eprintln!("report: {}", out.display());
}

/// Installs Gitleaks from its latest GitHub release into a throwaway tools folder,
/// then scans this repository with it. Downloads about 8 MB.
#[test]
#[ignore = "downloads Gitleaks from GitHub"]
fn installs_gitleaks_and_scans_with_it() {
    let tools = tempfile::tempdir().unwrap();
    let locator = security::Locator::new(tools.path());
    let def = security::install::def("gitleaks").unwrap();
    let cancel = AtomicBool::new(false);
    let exe = security::install::install(def, &locator, &Plain, &cancel, |u| match u {
        security::install::Update::Stage(s) => eprintln!("== {s}"),
        security::install::Update::Line(l) => eprintln!("   {l}"),
        security::install::Update::Progress(Some(p)) => eprintln!("   {:.0}%", p * 100.0),
        security::install::Update::Progress(None) => {}
    })
    .unwrap_or_else(|e| panic!("install failed: {e}"));
    eprintln!("installed: {}", exe.display());
    assert_eq!(locator.locate(def), Some(exe.clone()));
    assert!(security::install::installed_version(&locator, def).is_some());

    let root = std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let req = ScanRequest {
        project_id: "selftest",
        project_name: "devdock",
        dir: &root,
        options: ScanOptions::default(),
        previous: None,
        locator: &locator,
    };
    let report = security::scan(&req, &Plain, &cancel, |_| {});
    let leaks = report.tools.iter().find(|t| t.tool == "gitleaks").unwrap();
    eprintln!(
        "gitleaks: {:?} {} findings in {} ms {:?}",
        leaks.status, leaks.findings, leaks.ms, leaks.detail
    );
    for f in report
        .findings
        .iter()
        .filter(|f| f.kind == security::FindingKind::Secret)
    {
        eprintln!(
            "  {} {}:{}",
            f.id,
            f.location.as_deref().unwrap_or("?"),
            f.line.unwrap_or(0)
        );
    }
    assert_eq!(leaks.status, ToolStatus::Ran, "{:?}", leaks.detail);

    // A planted (fake) token is found in the code but not in node_modules, and
    // its value never reaches the report.
    let project = tempfile::tempdir().unwrap();
    // Built at runtime so this file doesn't contain a token-shaped string itself.
    let token = format!("{}_{}", "ghp", "R7fK2mQ9xLw4TzB8nV1cY6hJ3pD5sA0eGu2i");
    let token = token.as_str();
    std::fs::create_dir_all(project.path().join("src")).unwrap();
    std::fs::create_dir_all(project.path().join("node_modules/pkg")).unwrap();
    let line = format!("const token = \"{token}\";\n");
    std::fs::write(
        project.path().join("src/config.js"),
        format!("// config\n{line}"),
    )
    .unwrap();
    std::fs::write(project.path().join("node_modules/pkg/index.js"), &line).unwrap();
    let req = ScanRequest {
        project_id: "planted",
        project_name: "planted",
        dir: project.path(),
        options: ScanOptions::default(),
        previous: None,
        locator: &locator,
    };
    let report = security::scan(&req, &Plain, &cancel, |_| {});
    let secrets: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.kind == security::FindingKind::Secret)
        .collect();
    eprintln!("planted: {secrets:?}");
    assert_eq!(secrets.len(), 1, "{secrets:?}");
    assert_eq!(secrets[0].location.as_deref(), Some("src/config.js"));
    assert_eq!(secrets[0].line, Some(2));
    assert!(!serde_json::to_string(&report).unwrap().contains(token));

    security::install::uninstall(def, &locator).unwrap();
    assert!(!exe.exists());
}
