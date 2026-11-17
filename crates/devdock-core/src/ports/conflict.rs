//! Pre-launch port check (T5.5): before starting a command whose port DevDock knows,
//! see whether something already listens there.

use std::collections::HashSet;

use serde::Serialize;

use super::ownership::PortRow;
use crate::run::RunKey;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub port: u16,
    /// Who holds it now.
    pub holder: PortRow,
    /// A nearby free port to offer via `PORT=<n>`, if one exists.
    pub suggested_port: Option<u16>,
}

/// `None` = free to start. A port held by the same run (it's already running) isn't
/// a conflict; the start itself will report "already running".
pub fn check_port(port: u16, rows: &[PortRow], starting: &RunKey) -> Option<Conflict> {
    let holder = rows
        .iter()
        .find(|r| r.port == port && r.run.as_ref() != Some(starting))?;
    let taken: HashSet<u16> = rows.iter().map(|r| r.port).collect();
    Some(Conflict {
        port,
        holder: holder.clone(),
        suggested_port: free_port_near(port, &taken),
    })
}

/// The first port above `port` (within 100) that nothing listens on.
pub fn free_port_near(port: u16, taken: &HashSet<u16>) -> Option<u16> {
    (1..=100u16)
        .filter_map(|d| port.checked_add(d))
        .find(|p| !taken.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Owner;
    use crate::project::ProjectId;
    use crate::run::CommandId;

    fn row(port: u16, run: Option<RunKey>) -> PortRow {
        PortRow {
            port,
            addresses: vec!["0.0.0.0".into()],
            pid: 7,
            owner: if run.is_some() {
                Owner::DevDock
            } else {
                Owner::External
            },
            process: None,
            children: vec![],
            project: None,
            run,
        }
    }

    fn key(c: &str) -> RunKey {
        RunKey {
            project_id: ProjectId("p".into()),
            command_id: CommandId(c.into()),
        }
    }

    #[test]
    fn taken_port_is_a_conflict_with_a_suggestion() {
        let rows = [row(3000, None), row(3001, None)];
        let c = check_port(3000, &rows, &key("dev")).unwrap();
        assert_eq!(c.port, 3000);
        assert_eq!(c.suggested_port, Some(3002));
    }

    #[test]
    fn free_port_is_fine() {
        assert!(check_port(3000, &[row(4000, None)], &key("dev")).is_none());
    }

    #[test]
    fn our_own_run_holding_it_is_not_a_conflict() {
        assert!(check_port(3000, &[row(3000, Some(key("dev")))], &key("dev")).is_none());
        // ...but another DevDock run on that port is.
        assert!(check_port(3000, &[row(3000, Some(key("other")))], &key("dev")).is_some());
    }

    #[test]
    fn no_suggestion_past_the_top() {
        let taken: HashSet<u16> = [65535].into();
        assert_eq!(free_port_near(65535, &taken), None);
    }
}
