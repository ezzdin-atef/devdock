//! Process details (T5.2) and termination for confirmed external processes (T5.6).
//! Names, parents, start times, command lines and folders come from `sysinfo`;
//! session ids and termination are direct Win32 calls.
#![allow(unsafe_code)]

use std::collections::HashSet;
use std::io;
use std::sync::Mutex;

use devdock_core::ports::{Listener, Permit, ProcInfo, SystemProbe, TerminateError};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

/// Exit code given to processes ended from the Ports screen.
const ENDED_EXIT_CODE: u32 = 1;

pub struct WinProbe {
    sys: Mutex<Option<System>>,
}

impl Default for WinProbe {
    fn default() -> Self {
        Self {
            sys: Mutex::new(System::new().ok()),
        }
    }
}

fn session_of(pid: u32) -> Option<u32> {
    let mut session = 0u32;
    // SAFETY: `session` is a valid out-pointer for the duration of the call.
    unsafe { ProcessIdToSessionId(pid, &raw mut session) }
        .ok()
        .map(|()| session)
}

impl SystemProbe for WinProbe {
    fn listeners(&self) -> io::Result<Vec<Listener>> {
        super::ports::tcp_listeners()
    }

    fn processes(&self, detail_for: &[u32]) -> Vec<ProcInfo> {
        let mut guard = self.sys.lock().unwrap_or_else(|p| p.into_inner());
        let Some(sys) = guard.as_mut() else {
            return Vec::new();
        };
        // Cheap pass for everyone (name, parent, start time)...
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        // ...and the slow fields only where they are shown.
        let detail: HashSet<u32> = detail_for.iter().copied().collect();
        let pids: Vec<Pid> = detail.iter().map(|&p| Pid::from_u32(p)).collect();
        if !pids.is_empty() {
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&pids),
                false,
                ProcessRefreshKind::nothing()
                    .with_cmd(UpdateKind::Always)
                    .with_cwd(UpdateKind::Always)
                    .with_exe(UpdateKind::OnlyIfNotSet),
            );
        }

        sys.processes()
            .values()
            .map(|p| {
                let pid = p.pid().as_u32();
                let detailed = detail.contains(&pid);
                let cmd = p
                    .cmd()
                    .iter()
                    .map(|a| a.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                ProcInfo {
                    pid,
                    parent: p.parent().map(Pid::as_u32),
                    name: p.name().to_string_lossy().into_owned(),
                    exe: detailed.then(|| p.exe().map(Into::into)).flatten(),
                    // Empty means Windows wouldn't tell us (access denied): unknown.
                    cmd: (detailed && !cmd.is_empty()).then_some(cmd),
                    cwd: detailed.then(|| p.cwd().map(Into::into)).flatten(),
                    start_time: Some(p.start_time()).filter(|&t| t > 0),
                    session: session_of(pid),
                }
            })
            .collect()
    }

    fn terminate(&self, permit: &Permit) -> Result<(), TerminateError> {
        let pid = permit.pid();
        let name = || {
            self.processes(&[])
                .into_iter()
                .find(|p| p.pid == pid)
                .map_or_else(|| format!("PID {pid}"), |p| p.name)
        };
        // SAFETY: opening a process by id has no memory-safety preconditions.
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }.map_err(|e| {
            if e.code() == ERROR_INVALID_PARAMETER.to_hresult() {
                TerminateError::Gone(pid)
            } else if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
                TerminateError::AccessDenied { pid, name: name() }
            } else {
                TerminateError::Other(io::Error::from_raw_os_error(e.code().0))
            }
        })?;
        // SAFETY: `handle` was just opened with PROCESS_TERMINATE access.
        let result = unsafe { TerminateProcess(handle, ENDED_EXIT_CODE) };
        // SAFETY: `handle` is owned here and closed exactly once.
        unsafe {
            let _ = CloseHandle(handle);
        }
        result.map_err(|e| {
            if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
                TerminateError::AccessDenied { pid, name: name() }
            } else {
                TerminateError::Other(io::Error::from_raw_os_error(e.code().0))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn reads_our_own_process_details() {
        let probe = WinProbe::default();
        let me = std::process::id();
        let procs = probe.processes(&[me]);
        let mine = procs.iter().find(|p| p.pid == me).unwrap();
        assert!(mine.cmd.is_some());
        assert_eq!(
            mine.cwd.as_deref().map(devdock_core::paths::key),
            Some(devdock_core::paths::key(&std::env::current_dir().unwrap()))
        );
        assert!(mine.start_time.is_some());
        assert_ne!(mine.session, Some(0));
        // Details are only read where asked.
        assert!(
            procs
                .iter()
                .filter(|p| p.pid != me)
                .all(|p| p.cmd.is_none())
        );
    }

    #[test]
    fn services_classify_as_system() {
        use devdock_core::ports::{Owner, classify};
        let probe = WinProbe::default();
        let procs = probe.processes(&[]);
        // The service control manager always runs as a session-0 service.
        let services = procs
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case("services.exe"))
            .unwrap();
        assert_ne!(services.session, Some(1));
        assert_eq!(
            classify(services.pid, Some(services), &Default::default()),
            Owner::System
        );
        // And our own process is not system.
        let me = procs.iter().find(|p| p.pid == std::process::id()).unwrap();
        assert_eq!(
            classify(me.pid, Some(me), &Default::default()),
            Owner::External
        );
    }

    #[test]
    fn terminating_a_missing_pid_reports_gone_or_denied() {
        // PIDs are multiples of 4 on Windows, so an odd PID never exists.
        let probe = WinProbe::default();
        let r = devdock_core::ports::confirm(
            &probe,
            &devdock_core::ports::ConfirmRequest {
                pid: 999_999_997,
                start_time: None,
                with_children: false,
            },
            &Default::default(),
        );
        assert!(matches!(r, Err(devdock_core::ports::ConfirmError::Gone(_))));
    }

    #[test]
    fn confirmed_external_process_is_ended() {
        let mut child = Command::new("ping")
            .args(["-n", "60", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let probe = WinProbe::default();
        let start = probe
            .processes(&[child.id()])
            .into_iter()
            .find(|p| p.pid == child.id())
            .and_then(|p| p.start_time);
        let ok = devdock_core::ports::confirm(
            &probe,
            &devdock_core::ports::ConfirmRequest {
                pid: child.id(),
                start_time: start,
                with_children: true,
            },
            &Default::default(),
        )
        .unwrap();
        let ended = devdock_core::ports::terminate_external(&probe, ok).unwrap();
        assert_eq!(ended.pids.last(), Some(&child.id()));
        assert_eq!(child.wait().unwrap().code(), Some(ENDED_EXIT_CODE as i32));
    }
}
