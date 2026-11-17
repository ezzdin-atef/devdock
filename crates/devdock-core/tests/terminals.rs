//! Opens real terminal windows, so these are opt-in:
//! `cargo test -p devdock-core --test terminals -- --ignored`
//!
//! They run a harmless `touch` instead of an agent and check the marker lands in the
//! requested folder, then close the window they opened.

#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use devdock_core::agents::{LaunchSpec, TerminalKind, plan};
use devdock_core::launcher::spawn_plan_child;
use devdock_core::tools::ToolScan;

fn wait_for(p: &Path) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if p.exists() {
            return true;
        }
        thread::sleep(Duration::from_millis(200));
    }
    false
}

fn run_in(terminal: TerminalKind, program: &str) {
    let tools = ToolScan::detect();
    if !terminal.available(&tools) {
        eprintln!("{} not installed; skipping", terminal.name());
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // The program creates a file named after its last word.
    let marker = dir.path().join(program.rsplit(' ').next().unwrap());
    let spec = LaunchSpec {
        terminal,
        cwd: dir.path().to_path_buf(),
        program: Some(program.into()),
    };
    let mut child = spawn_plan_child(&plan(&spec, &tools).unwrap(), terminal.name()).unwrap();
    let found = wait_for(&marker);
    // Close the window this test opened (the whole tree under it).
    let _ = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .output();
    let _ = child.wait();
    assert!(
        found,
        "{} didn't run in {}",
        terminal.name(),
        dir.path().display()
    );
}

#[test]
#[ignore = "opens a Git Bash window"]
fn git_bash_starts_in_the_requested_folder() {
    run_in(TerminalKind::GitBash, "touch devdock-cwd-marker");
}

/// `wt.exe` hands the tab to Windows Terminal and exits, so the shell isn't our
/// child: it is found by its unique command line and closed afterwards.
#[test]
#[ignore = "opens a Windows Terminal window"]
fn windows_terminal_starts_in_the_requested_folder() {
    run_in(
        TerminalKind::WindowsTerminal,
        "New-Item devdock-wt-cwd-marker",
    );
    let marker_shell = "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*devdock-wt-cwd-marker*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }";
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", marker_shell])
        .output();
}

#[test]
#[ignore = "opens a PowerShell window"]
fn powershell_starts_in_the_requested_folder() {
    run_in(TerminalKind::PowerShell, "New-Item devdock-cwd-marker");
}
