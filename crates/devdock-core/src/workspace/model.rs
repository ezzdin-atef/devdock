//! Workspaces (T7.1): named groups of services from one or more projects, started
//! in order. Saved in `devdock.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::{AgentId, TerminalKind};
use crate::project::ProjectId;
use crate::run::CommandId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default)]
    pub restart: RestartPolicy,
    #[serde(default)]
    pub after_start: AfterStart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    /// Short id, unique within the workspace.
    pub id: String,
    pub name: String,
    /// Start position; lower starts first. Ties break by name.
    pub order: u32,
    pub source: ServiceSource,
    /// The port it should listen on (conflict check, TCP health default, display).
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub health: Option<Health>,
    /// Service ids that must be healthy first. Optional; `order` is enough usually.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ServiceSource {
    /// A command from the project's Run tab, optionally in one of its Git worktrees (T7.7).
    Command {
        project_id: ProjectId,
        command_id: CommandId,
        #[serde(default)]
        worktree: Option<PathBuf>,
    },
    /// One service of a Docker Compose file in the project (T7.6).
    Compose {
        project_id: ProjectId,
        /// Relative to the project folder.
        file: String,
        service: String,
    },
}

impl ServiceSource {
    pub fn project_id(&self) -> &ProjectId {
        match self {
            ServiceSource::Command { project_id, .. }
            | ServiceSource::Compose { project_id, .. } => project_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Health {
    /// Healthy when something accepts a TCP connection on localhost:port.
    Tcp { port: u16 },
    /// Healthy when `GET url` answers with `expect` (any 2xx/3xx if `None`). http only.
    Http { url: String, expect: Option<u16> },
}

/// Restart on crash, at most `max` times within `window_secs` (T7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RestartPolicy {
    pub enabled: bool,
    pub max: u32,
    pub window_secs: u64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max: 3,
            window_secs: 300,
        }
    }
}

/// "Open VS Code + agent after start" (T7.8).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AfterStart {
    /// Project to open; `None` disables the preset.
    pub project_id: Option<ProjectId>,
    pub open_editor: bool,
    pub agent: Option<AgentId>,
    pub terminal: TerminalKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_and_health_are_tagged_for_the_ui() {
        let s = Service {
            id: "api".into(),
            name: "API".into(),
            order: 2,
            source: ServiceSource::Compose {
                project_id: ProjectId("p".into()),
                file: "docker-compose.yml".into(),
                service: "db".into(),
            },
            port: Some(5432),
            health: Some(Health::Http {
                url: "http://localhost:5432/".into(),
                expect: Some(200),
            }),
            depends_on: vec![],
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["source"]["kind"], "compose");
        assert_eq!(v["source"]["projectId"], "p");
        assert_eq!(v["health"]["kind"], "http");
        let back: Service = serde_json::from_value(v).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn old_files_get_default_policy() {
        let w: Workspace = serde_json::from_str(r#"{"id":"w","name":"Wallet"}"#).unwrap();
        assert_eq!(w.restart, RestartPolicy::default());
        assert!(w.services.is_empty());
        assert!(w.after_start.project_id.is_none());
    }
}
