//! Workspaces (M7): start a set of services in order, wait for health, restart on
//! crash, stop in reverse, and show their logs together.

pub mod driver;
pub mod health;
pub mod logs;
pub mod model;
pub mod orchestrator;
pub mod order;

pub use model::{
    AfterStart, Health, RestartPolicy, Service, ServiceSource, Workspace, WorkspaceId,
};
pub use orchestrator::{Orchestrator, OrchestratorView, Phase, ServiceDriver, SvcState, Timing};
pub use order::{OrderError, start_order};
