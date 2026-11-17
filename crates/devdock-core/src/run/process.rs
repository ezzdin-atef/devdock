//! The OS boundary for running commands (T4.2a). Core only knows this trait; the
//! Windows Job Object implementation lives in `devdock-platform`, and tests use a
//! plain implementation. Trait objects (`Arc<dyn ProcessTree>`) because the choice
//! is made once at startup and the supervisor shouldn't be generic over it.

use std::io;
use std::path::PathBuf;
use std::process::Child;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    /// Passed verbatim to the shell (`cmd /D /S /C "<line>"` on Windows).
    pub command_line: String,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

/// A started process tree. `child` is the shell; its stdout and stderr must be piped.
pub struct Spawned {
    pub pid: u32,
    pub child: Child,
    pub tree: Box<dyn TreeHandle>,
}

/// Controls every process started by one spawn, including grandchildren.
pub trait TreeHandle: Send + Sync {
    /// Ends the whole tree. Safe to call more than once.
    fn kill_tree(&self) -> io::Result<()>;
    /// How many processes are alive in the tree, when the OS can tell.
    fn process_count(&self) -> Option<usize>;
    /// PIDs currently in the tree. This is what makes a port "owned by DevDock":
    /// membership, never a matching name or port.
    fn pids(&self) -> Vec<u32> {
        Vec::new()
    }
}

pub trait ProcessTree: Send + Sync {
    fn spawn(&self, spec: &SpawnSpec) -> io::Result<Spawned>;
}
