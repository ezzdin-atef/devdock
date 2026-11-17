//! Who owns a listening process (T5.3), and the rows the Ports screen shows.
//!
//! Rules, in order:
//! 1. `DevDock` only if the PID is inside one of DevDock's job objects.
//! 2. `System` for PID 0 (idle) and 4 (kernel), for session-0 processes (services),
//!    and for processes whose session Windows won't even tell us: they belong to
//!    another account (or are elevated), so DevDock couldn't end them anyway.
//! 3. Everything else is `External`.
//!
//! A project is matched by the process's working folder (longest prefix), never by
//! its name or port number.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Serialize;

use super::{Listener, ProcInfo};
use crate::paths;
use crate::project::{Project, ProjectId};
use crate::run::RunKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Owner {
    DevDock,
    System,
    External,
}

pub fn classify(pid: u32, info: Option<&ProcInfo>, owned: &HashMap<u32, RunKey>) -> Owner {
    if owned.contains_key(&pid) {
        Owner::DevDock
    } else if pid == 0 || pid == 4 || info.is_some_and(|i| i.session.is_none_or(|s| s == 0)) {
        Owner::System
    } else {
        Owner::External
    }
}

/// The project whose folder contains `cwd`; the deepest one wins.
pub fn match_project<'a>(cwd: &Path, projects: &'a [Project]) -> Option<&'a Project> {
    let cwd = paths::key(cwd);
    projects
        .iter()
        .filter(|p| {
            let root = paths::key(&p.path);
            let root = root.trim_end_matches('\\');
            cwd == root || cwd.starts_with(&format!("{root}\\"))
        })
        .max_by_key(|p| p.path.as_os_str().len())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildProc {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    pub id: ProjectId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortRow {
    pub port: u16,
    /// e.g. `0.0.0.0`, `127.0.0.1`, `::`; one row per (port, pid).
    pub addresses: Vec<String>,
    pub pid: u32,
    pub owner: Owner,
    pub process: Option<ProcInfo>,
    pub children: Vec<ChildProc>,
    pub project: Option<ProjectRef>,
    /// Set when DevDock owns it: the run to stop.
    pub run: Option<RunKey>,
}

pub fn build_rows(
    listeners: &[Listener],
    procs: &[ProcInfo],
    owned: &HashMap<u32, RunKey>,
    projects: &[Project],
) -> Vec<PortRow> {
    let by_pid: HashMap<u32, &ProcInfo> = procs.iter().map(|p| (p.pid, p)).collect();

    let mut grouped: BTreeMap<(u16, u32), Vec<String>> = BTreeMap::new();
    for l in listeners {
        let addrs = grouped.entry((l.port, l.pid)).or_default();
        let a = l.address.to_string();
        if !addrs.contains(&a) {
            addrs.push(a);
        }
    }

    grouped
        .into_iter()
        .map(|((port, pid), addresses)| {
            let info = by_pid.get(&pid).copied();
            let owner = classify(pid, info, owned);
            let run = owned.get(&pid).cloned();
            let project = match &run {
                Some(key) => projects.iter().find(|p| p.id == key.project_id),
                None => info
                    .and_then(|i| i.cwd.as_deref())
                    .and_then(|cwd| match_project(cwd, projects)),
            }
            .map(|p| ProjectRef {
                id: p.id.clone(),
                name: p.name.clone(),
            });
            let children = procs
                .iter()
                .filter(|c| c.parent == Some(pid) && started_after(c, info))
                .map(|c| ChildProc {
                    pid: c.pid,
                    name: c.name.clone(),
                })
                .collect();
            PortRow {
                port,
                addresses,
                pid,
                owner,
                process: info.cloned(),
                children,
                project,
                run,
            }
        })
        .collect()
}

/// Windows reuses PIDs, so a "child" whose parent PID matches but which started
/// before the parent is really an orphan of an older process.
pub(crate) fn started_after(child: &ProcInfo, parent: Option<&ProcInfo>) -> bool {
    match (child.start_time, parent.and_then(|p| p.start_time)) {
        (Some(c), Some(p)) => c >= p,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::{Detected, ProjectKind};
    use crate::run::CommandId;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::path::PathBuf;

    fn proc(pid: u32, name: &str, cwd: Option<&str>, session: u32) -> ProcInfo {
        ProcInfo {
            pid,
            parent: None,
            name: name.into(),
            exe: None,
            cmd: Some(format!("{name} serve")),
            cwd: cwd.map(PathBuf::from),
            start_time: Some(100),
            session: Some(session),
        }
    }

    fn project(path: &str) -> Project {
        Project::new(
            PathBuf::from(path),
            &Detected {
                kind: ProjectKind::Node,
                markers: vec![],
                is_git: false,
            },
            0,
        )
    }

    fn key(p: &Project) -> RunKey {
        RunKey {
            project_id: p.id.clone(),
            command_id: CommandId("script:dev".into()),
        }
    }

    fn listen(port: u16, pid: u32) -> Listener {
        Listener {
            port,
            pid,
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        }
    }

    #[test]
    fn same_name_process_outside_our_job_is_external() {
        let web = project("Q:\\dev\\web");
        let owned: HashMap<u32, RunKey> = [(100, key(&web))].into();
        // Both are "node.exe" in the same folder; only PID 100 is in our job.
        let procs = [
            proc(100, "node.exe", Some("Q:\\dev\\web"), 1),
            proc(200, "node.exe", Some("Q:\\dev\\web"), 1),
        ];
        assert_eq!(classify(100, Some(&procs[0]), &owned), Owner::DevDock);
        assert_eq!(classify(200, Some(&procs[1]), &owned), Owner::External);
    }

    #[test]
    fn kernel_idle_and_services_are_system() {
        let none = HashMap::new();
        assert_eq!(classify(0, None, &none), Owner::System);
        assert_eq!(classify(4, None, &none), Owner::System);
        let svc = proc(900, "svchost.exe", None, 0);
        assert_eq!(classify(900, Some(&svc), &none), Owner::System);
        // Session hidden from us: another account's process, protected.
        let mut hidden = proc(902, "MsMpEng.exe", None, 1);
        hidden.session = None;
        assert_eq!(classify(902, Some(&hidden), &none), Owner::System);
        // No info at all (it exited between two reads) is not a reason to call it "system".
        assert_eq!(classify(901, None, &none), Owner::External);
    }

    #[test]
    fn project_match_uses_the_folder_not_the_name() {
        let projects = [
            project("Q:\\dev\\web"),
            project("Q:\\dev\\web\\apps\\admin"),
            project("Q:\\dev\\webshop"),
        ];
        let hit = |cwd: &str| match_project(Path::new(cwd), &projects).map(|p| p.name.clone());
        assert_eq!(hit("Q:\\dev\\web"), Some("web".into()));
        assert_eq!(hit("q:\\DEV\\web\\src"), Some("web".into()));
        assert_eq!(
            hit("Q:\\dev\\web\\apps\\admin\\.next"),
            Some("admin".into())
        );
        // `webshop` must not match the `web` project by prefix.
        assert_eq!(hit("Q:\\dev\\webshop"), Some("webshop".into()));
        assert_eq!(hit("Q:\\elsewhere\\web"), None);
    }

    #[test]
    fn rows_merge_addresses_and_attach_owner_project_children() {
        let web = project("Q:\\dev\\web");
        let api = project("Q:\\dev\\api");
        let owned: HashMap<u32, RunKey> = [(100, key(&web))].into();
        let mut child = proc(101, "esbuild.exe", None, 1);
        child.parent = Some(100);
        child.start_time = Some(150);
        let mut stale = proc(102, "old.exe", None, 1);
        stale.parent = Some(100);
        stale.start_time = Some(50); // started before its "parent": PID reuse, not a child
        let procs = [
            proc(100, "node.exe", Some("Q:\\dev\\web"), 1),
            proc(300, "api.exe", Some("Q:\\dev\\api\\target"), 1),
            child,
            stale,
        ];
        let listeners = [
            listen(3000, 100),
            Listener {
                port: 3000,
                pid: 100,
                address: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            },
            listen(8080, 300),
            listen(135, 4),
        ];
        let rows = build_rows(&listeners, &procs, &owned, &[web, api]);
        assert_eq!(
            rows.iter().map(|r| r.port).collect::<Vec<_>>(),
            vec![135, 3000, 8080]
        );

        let web_row = &rows[1];
        assert_eq!(web_row.owner, Owner::DevDock);
        assert_eq!(web_row.addresses, vec!["127.0.0.1", "::"]);
        assert_eq!(web_row.project.as_ref().unwrap().name, "web");
        assert_eq!(
            web_row.children,
            vec![ChildProc {
                pid: 101,
                name: "esbuild.exe".into()
            }]
        );
        assert!(web_row.run.is_some());

        let api_row = &rows[2];
        assert_eq!(api_row.owner, Owner::External);
        assert_eq!(api_row.project.as_ref().unwrap().name, "api");
        assert!(api_row.run.is_none());

        assert_eq!(rows[0].owner, Owner::System);
        assert!(rows[0].process.is_none());
    }
}
