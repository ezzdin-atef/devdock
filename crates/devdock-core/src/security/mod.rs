//! Security scans: run the real scanners a developer would (Grype, Trivy,
//! OSV-Scanner, the package manager's audit, cargo audit, Gitleaks, Semgrep), merge what they report into one list,
//! compare with the previous scan, and render a report.
//!
//! DevDock scans nothing itself. Scanners missing from PATH are listed as skipped,
//! with how to install them, so a scan never fails for lack of a tool.

pub mod history;
pub mod html;
pub mod install;
pub mod merge;
pub mod model;
pub mod parse;
pub mod scanners;
pub mod version;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use serde::Serialize;

pub use history::History;
pub use model::{
    Counts, Finding, FindingKind, Report, ScanOptions, Severity, Summary, ToolRun, ToolStatus,
};
pub use scanners::{Locator, ScannerInfo, detect};

use crate::run::ProcessTree;
use crate::time::now_ms;
use scanners::Outcome;

pub const REPORT_VERSION: u32 = 1;

/// Sent while a scan runs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub project_id: String,
    /// 1-based index of the scanner now running.
    pub step: usize,
    pub total: usize,
    pub tool: String,
    pub name: String,
    pub started_at: u64,
}

pub struct ScanRequest<'a> {
    pub project_id: &'a str,
    pub project_name: &'a str,
    pub dir: &'a Path,
    pub options: ScanOptions,
    pub previous: Option<&'a Report>,
    /// Where DevDock's own copies of scanners live.
    pub locator: &'a Locator,
}

/// Runs every applicable scanner in turn. Blocking: call it off the UI thread.
pub fn scan(
    req: &ScanRequest<'_>,
    tree: &dyn ProcessTree,
    cancel: &AtomicBool,
    progress: impl Fn(&Progress),
) -> Report {
    let started_at = now_ms();
    let report_file = std::env::temp_dir().join(format!(
        "devdock-gitleaks-{}-{started_at}.json",
        std::process::id()
    ));
    let steps = scanners::plan(
        req.dir,
        req.options,
        &report_file,
        |def| req.locator.locate(def),
        |p| crate::tools::which(p).is_some(),
    );
    let runnable = steps.iter().filter(|s| s.command_line.is_some()).count();

    let mut tools = Vec::new();
    let mut found = Vec::new();
    let mut index = 0;
    let mut cancelled = false;
    for step in &steps {
        let mut run = ToolRun {
            tool: step.tool.clone(),
            name: step.name.clone(),
            status: ToolStatus::Skipped,
            detail: step.skip.clone(),
            install: step.install.clone(),
            command: step.command_line.clone(),
            findings: 0,
            ms: 0,
        };
        if step.command_line.is_some() {
            if cancelled || cancel.load(std::sync::atomic::Ordering::Relaxed) {
                cancelled = true;
                run.status = ToolStatus::Cancelled;
                run.detail = Some("The scan was stopped before this ran.".into());
                tools.push(run);
                continue;
            }
            index += 1;
            progress(&Progress {
                project_id: req.project_id.to_owned(),
                step: index,
                total: runnable,
                tool: step.tool.clone(),
                name: step.name.clone(),
                started_at: now_ms(),
            });
            let t = Instant::now();
            let outcome = scanners::run_step(tree, req.dir, step, cancel);
            run.ms = u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX);
            match outcome {
                Outcome::Ran { findings } => {
                    run.status = ToolStatus::Ran;
                    run.findings = findings.len();
                    run.detail = step.note.clone();
                    found.extend(findings);
                }
                Outcome::Failed(msg) => {
                    run.status = ToolStatus::Failed;
                    run.detail = Some(msg);
                }
                Outcome::Cancelled => {
                    cancelled = true;
                    run.status = ToolStatus::Cancelled;
                    run.detail = Some("Stopped.".into());
                }
            }
        }
        tools.push(run);
    }

    let mut findings = merge::merge(found);
    // A cancelled scan didn't look everywhere, so "fixed" would be a guess.
    let previous = if cancelled { None } else { req.previous };
    let resolved = merge::compare(&mut findings, previous.map(|p| p.findings.as_slice()));
    let counts = Counts::of(&findings);
    let upgrades = merge::upgrades(&findings);
    Report {
        version: REPORT_VERSION,
        project_id: req.project_id.to_owned(),
        project_name: req.project_name.to_owned(),
        path: req.dir.to_string_lossy().into_owned(),
        started_at,
        finished_at: now_ms(),
        options: req.options,
        tools,
        findings,
        counts,
        upgrades,
        previous_at: previous.map(|p| p.finished_at),
        resolved,
        cancelled,
    }
}
