//! DevDock's domain logic. Everything here is plain Rust: no Tauri, no UI types,
//! so it can be tested without a window. The `devdock` app crate only adapts it.

pub mod activity;
pub mod agents;
pub mod detect;
pub mod error;
pub mod git;
pub mod git_job;
pub mod launcher;
pub mod library;
pub mod palette;
pub mod paths;
pub mod ports;
pub mod prefs;
pub mod project;
pub mod run;
pub mod saver;
pub mod scan;
pub mod scan_job;
pub mod state;
pub mod store;
pub mod time;
pub mod tools;
pub mod workspace;
