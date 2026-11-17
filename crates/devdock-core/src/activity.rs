//! Recent activity per project (T8.1): opened, started, ended, agent launched.
//! Saved with the project, newest first, capped.

use serde::{Deserialize, Serialize};

use crate::agents::{AgentId, TerminalKind};

pub const KEEP_ACTIVITY: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Activity {
    OpenedInEditor {
        editor: String,
        at: u64,
    },
    OpenedInExplorer {
        at: u64,
    },
    Started {
        command: String,
        at: u64,
    },
    /// A run ended. `outcome` is "stopped", "exited" or "failed".
    Ended {
        command: String,
        outcome: String,
        code: Option<i32>,
        at: u64,
    },
    AgentLaunched {
        agent: Option<AgentId>,
        terminal: TerminalKind,
        at: u64,
    },
}

impl Activity {
    pub fn at(&self) -> u64 {
        match self {
            Activity::OpenedInEditor { at, .. }
            | Activity::OpenedInExplorer { at }
            | Activity::Started { at, .. }
            | Activity::Ended { at, .. }
            | Activity::AgentLaunched { at, .. } => *at,
        }
    }
}

/// Adds `entry` to a newest-first log and keeps it within `KEEP_ACTIVITY`.
pub fn push(log: &mut Vec<Activity>, entry: Activity) {
    log.insert(0, entry);
    log.truncate(KEEP_ACTIVITY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_and_capped() {
        let mut log = Vec::new();
        for i in 0..(KEEP_ACTIVITY as u64 + 5) {
            push(&mut log, Activity::OpenedInExplorer { at: i });
        }
        assert_eq!(log.len(), KEEP_ACTIVITY);
        assert_eq!(log[0].at(), KEEP_ACTIVITY as u64 + 4);
    }

    #[test]
    fn serializes_with_kind_tags() {
        let v = serde_json::to_value(Activity::Ended {
            command: "npm run dev".into(),
            outcome: "failed".into(),
            code: Some(1),
            at: 5,
        })
        .unwrap();
        assert_eq!(v["kind"], "ended");
        assert_eq!(v["outcome"], "failed");
    }
}
