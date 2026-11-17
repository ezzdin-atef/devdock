//! Port inspector (M5): who listens on which TCP port, who owns that process, and
//! the only path for ending a process DevDock didn't start.

pub mod confirm;
pub mod conflict;
pub mod ownership;

use std::io;
use std::net::IpAddr;
use std::path::PathBuf;

use serde::Serialize;

pub use confirm::{
    ConfirmError, ConfirmRequest, Confirmed, Ended, Permit, confirm, terminate_external,
};
pub use conflict::{Conflict, check_port, free_port_near};
pub use ownership::{Owner, PortRow, build_rows, classify, match_project};

/// One listening TCP socket (T5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub port: u16,
    pub pid: u32,
    pub address: IpAddr,
}

/// What DevDock could learn about a process (T5.2). Fields are `None` when Windows
/// refuses to tell (another user's or an elevated process): shown as "unknown".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub exe: Option<PathBuf>,
    pub cmd: Option<String>,
    pub cwd: Option<PathBuf>,
    /// Unix seconds. Also used to detect PID reuse.
    pub start_time: Option<u64>,
    /// Windows session; 0 = services.
    pub session: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum TerminateError {
    #[error("Windows denied access to {name} (PID {pid})")]
    AccessDenied { pid: u32, name: String },
    #[error("PID {0} has already exited")]
    Gone(u32),
    #[error("{0}")]
    Other(io::Error),
}

/// The OS boundary for ports and processes. Implemented in `devdock-platform`.
///
/// `terminate` takes a [`Permit`], which only this crate can create, and only via
/// [`confirm`]. So no caller, here or in another crate, can end a process without
/// going through the confirmation checks.
pub trait SystemProbe: Send + Sync {
    fn listeners(&self) -> io::Result<Vec<Listener>>;
    /// Every process with parent, name and start time; command line, folder and exe
    /// only for `detail_for` (reading those is slow).
    fn processes(&self, detail_for: &[u32]) -> Vec<ProcInfo>;
    fn terminate(&self, permit: &Permit) -> Result<(), TerminateError>;
}
