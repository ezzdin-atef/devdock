//! DevDock's OS boundary. Implements the traits defined in `devdock-core`.
//! All `unsafe` in DevDock lives in the `win` modules of this crate.

#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use win::{JobTree, WinProbe, run_elevated};

/// The process-tree implementation for this OS.
#[cfg(windows)]
pub fn process_tree() -> std::sync::Arc<dyn devdock_core::run::ProcessTree> {
    std::sync::Arc::new(JobTree)
}

/// The ports-and-processes probe for this OS.
#[cfg(windows)]
pub fn system_probe() -> std::sync::Arc<dyn devdock_core::ports::SystemProbe> {
    std::sync::Arc::new(WinProbe::default())
}
