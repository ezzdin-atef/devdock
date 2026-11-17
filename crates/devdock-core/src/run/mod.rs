//! Running and supervising project commands (M4).

pub mod logs;
pub mod port_hint;
pub mod process;
pub mod quick;
pub mod serve;
pub mod spec;
pub mod state;
pub mod supervisor;

pub use logs::{LogLine, LogsPage, Stream};
pub use process::{ProcessTree, SpawnSpec, Spawned, TreeHandle};
pub use quick::quick_shell;
pub use serve::{PortMode, RunPrefs};
pub use spec::{CommandId, CustomCommand, Discovered, RunCommand, discover, validate_custom};
pub use state::RunState;
pub use supervisor::{Change, Notify, RunError, RunInfo, RunKey, RunLabel, Runner};
