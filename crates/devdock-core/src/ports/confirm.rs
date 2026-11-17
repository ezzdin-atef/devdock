//! Ending a process DevDock didn't start (T5.6). The only way in is
//! [`confirm`] → [`Confirmed`] → [`terminate_external`]:
//!
//! * `Confirmed` can't be built by hand (private fields, no public constructor);
//! * `terminate_external` consumes it, so one confirmation ends one process (tree);
//! * the low-level [`SystemProbe::terminate`] needs a [`Permit`], which only this
//!   module can create.
//!
//! ```compile_fail
//! // A Confirmed token can't be forged outside devdock-core.
//! let forged = devdock_core::ports::Confirmed { pid: 1234, start_time: None, with_children: true, name: String::new() };
//! ```
//!
//! ```compile_fail
//! // Nor can a Permit, so a SystemProbe can't be told to kill a PID directly.
//! let permit = devdock_core::ports::Permit { pid: 1234 };
//! ```
//!
//! ```compile_fail
//! // terminate_external needs the token, not a PID.
//! fn end(probe: &dyn devdock_core::ports::SystemProbe) {
//!     let _ = devdock_core::ports::terminate_external(probe, 1234);
//! }
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::ownership::{Owner, classify, started_after};
use super::{ProcInfo, SystemProbe, TerminateError};
use crate::run::RunKey;

/// What the user saw in the dialog and agreed to. `start_time` pins the exact
/// process: if the PID was reused since the dialog opened, confirmation fails.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmRequest {
    pub pid: u32,
    pub start_time: Option<u64>,
    pub with_children: bool,
}

/// Proof that the user confirmed ending this exact process. See module docs.
#[derive(Debug)]
pub struct Confirmed {
    pid: u32,
    start_time: Option<u64>,
    with_children: bool,
    name: String,
}

/// Permission for one `SystemProbe::terminate` call. Only this module creates them.
#[derive(Debug)]
pub struct Permit {
    pid: u32,
}

impl Permit {
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfirmError {
    #[error("PID {0} is no longer running")]
    Gone(u32),
    #[error("PID {0} now belongs to a different process than the one you confirmed")]
    Changed(u32),
    #[error("{name} (PID {pid}) is a protected system process")]
    Protected { pid: u32, name: String },
    #[error("{name} (PID {pid}) was started by DevDock; use Stop instead")]
    Owned { pid: u32, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ended {
    /// Every PID that was ended, children first.
    pub pids: Vec<u32>,
    pub name: String,
}

/// Re-reads the process and checks it is still the one the user confirmed, is not
/// a system process, and isn't DevDock's own (those are stopped through their job).
pub fn confirm(
    probe: &dyn SystemProbe,
    request: &ConfirmRequest,
    owned: &HashMap<u32, RunKey>,
) -> Result<Confirmed, ConfirmError> {
    let procs = probe.processes(&[request.pid]);
    let info = procs
        .iter()
        .find(|p| p.pid == request.pid)
        .ok_or(ConfirmError::Gone(request.pid))?;
    if request.start_time.is_some() && info.start_time != request.start_time {
        return Err(ConfirmError::Changed(request.pid));
    }
    let name = info.name.clone();
    match classify(request.pid, Some(info), owned) {
        Owner::System => Err(ConfirmError::Protected {
            pid: request.pid,
            name,
        }),
        Owner::DevDock => Err(ConfirmError::Owned {
            pid: request.pid,
            name,
        }),
        Owner::External => Ok(Confirmed {
            pid: request.pid,
            start_time: info.start_time,
            with_children: request.with_children,
            name,
        }),
    }
}

/// Ends a confirmed external process, and its descendants if the user asked.
/// Children go first so they can't be re-parented and survive.
pub fn terminate_external(
    probe: &dyn SystemProbe,
    confirmed: Confirmed,
) -> Result<Ended, TerminateError> {
    let mut order = Vec::new();
    if confirmed.with_children {
        let procs = probe.processes(&[]);
        collect_descendants(confirmed.pid, confirmed.start_time, &procs, &mut order);
    }
    order.push(confirmed.pid);

    for &pid in &order {
        match probe.terminate(&Permit { pid }) {
            // A child that exited on its own meanwhile is fine.
            Err(TerminateError::Gone(_)) if pid != confirmed.pid => {}
            other => other?,
        }
    }
    Ok(Ended {
        pids: order,
        name: confirmed.name,
    })
}

/// Depth-first, deepest first, guarding against PID reuse and cycles.
fn collect_descendants(pid: u32, start: Option<u64>, procs: &[ProcInfo], out: &mut Vec<u32>) {
    let parent = ProcInfo {
        pid,
        start_time: start,
        ..ProcInfo::default()
    };
    for child in procs
        .iter()
        .filter(|c| c.parent == Some(pid) && c.pid != pid && started_after(c, Some(&parent)))
    {
        if out.contains(&child.pid) {
            continue;
        }
        collect_descendants(child.pid, child.start_time, procs, out);
        out.push(child.pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Listener;
    use crate::project::ProjectId;
    use crate::run::CommandId;
    use std::sync::Mutex;

    /// Fake OS: a fixed process list; records what it was asked to terminate.
    struct FakeProbe {
        procs: Vec<ProcInfo>,
        killed: Mutex<Vec<u32>>,
        deny: Option<u32>,
    }

    impl SystemProbe for FakeProbe {
        fn listeners(&self) -> std::io::Result<Vec<Listener>> {
            Ok(vec![])
        }
        fn processes(&self, _: &[u32]) -> Vec<ProcInfo> {
            self.procs.clone()
        }
        fn terminate(&self, permit: &Permit) -> Result<(), TerminateError> {
            if self.deny == Some(permit.pid()) {
                return Err(TerminateError::AccessDenied {
                    pid: permit.pid(),
                    name: "x".into(),
                });
            }
            self.killed.lock().unwrap().push(permit.pid());
            Ok(())
        }
    }

    fn p(pid: u32, parent: Option<u32>, start: u64, session: u32) -> ProcInfo {
        ProcInfo {
            pid,
            parent,
            name: format!("p{pid}.exe"),
            start_time: Some(start),
            session: Some(session),
            ..ProcInfo::default()
        }
    }

    fn probe(procs: Vec<ProcInfo>) -> FakeProbe {
        FakeProbe {
            procs,
            killed: Mutex::default(),
            deny: None,
        }
    }

    fn req(pid: u32, start: u64, with_children: bool) -> ConfirmRequest {
        ConfirmRequest {
            pid,
            start_time: Some(start),
            with_children,
        }
    }

    #[test]
    fn ends_children_first_and_skips_reused_pids() {
        // 10 -> 11 -> 12, and 13 claims parent 10 but started earlier (PID reuse).
        let fake = probe(vec![
            p(10, None, 100, 1),
            p(11, Some(10), 110, 1),
            p(12, Some(11), 120, 1),
            p(13, Some(10), 50, 1),
        ]);
        let ok = confirm(&fake, &req(10, 100, true), &HashMap::new()).unwrap();
        let ended = terminate_external(&fake, ok).unwrap();
        assert_eq!(ended.pids, vec![12, 11, 10]);
        assert_eq!(*fake.killed.lock().unwrap(), vec![12, 11, 10]);
    }

    #[test]
    fn without_children_only_the_process_ends() {
        let fake = probe(vec![p(10, None, 100, 1), p(11, Some(10), 110, 1)]);
        let ok = confirm(&fake, &req(10, 100, false), &HashMap::new()).unwrap();
        assert_eq!(terminate_external(&fake, ok).unwrap().pids, vec![10]);
    }

    #[test]
    fn a_reused_pid_is_refused() {
        let fake = probe(vec![p(10, None, 999, 1)]);
        assert!(matches!(
            confirm(&fake, &req(10, 100, false), &HashMap::new()),
            Err(ConfirmError::Changed(10))
        ));
    }

    #[test]
    fn gone_protected_and_owned_are_refused() {
        let fake = probe(vec![p(20, None, 1, 0), p(30, None, 1, 1)]);
        let owned: HashMap<u32, RunKey> = [(
            30,
            RunKey {
                project_id: ProjectId("p".into()),
                command_id: CommandId("c".into()),
            },
        )]
        .into();
        assert!(matches!(
            confirm(&fake, &req(99, 1, false), &owned),
            Err(ConfirmError::Gone(99))
        ));
        assert!(matches!(
            confirm(&fake, &req(20, 1, false), &owned),
            Err(ConfirmError::Protected { .. })
        ));
        assert!(matches!(
            confirm(&fake, &req(30, 1, false), &owned),
            Err(ConfirmError::Owned { .. })
        ));
        assert!(fake.killed.lock().unwrap().is_empty());
    }

    #[test]
    fn access_denied_is_reported() {
        let mut fake = probe(vec![p(10, None, 100, 1)]);
        fake.deny = Some(10);
        let ok = confirm(&fake, &req(10, 100, false), &HashMap::new()).unwrap();
        assert!(matches!(
            terminate_external(&fake, ok),
            Err(TerminateError::AccessDenied { pid: 10, .. })
        ));
    }
}
