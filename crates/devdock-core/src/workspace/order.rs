//! Start order (T7.2): by `order`, ties by name, and every `depends_on` before its
//! dependant. A dependency cycle or unknown id is an error, not a guess.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use super::model::Service;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrderError {
    #[error("{service} depends on {missing}, which isn't in this workspace")]
    UnknownDependency { service: String, missing: String },
    #[error("services depend on each other in a loop: {0}")]
    Cycle(String),
}

fn by_order(a: &Service, b: &Service) -> Ordering {
    a.order
        .cmp(&b.order)
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        .then_with(|| a.id.cmp(&b.id))
}

/// Indices into `services`, in the order they should start.
pub fn start_order(services: &[Service]) -> Result<Vec<usize>, OrderError> {
    let index: HashMap<&str, usize> = services
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    for s in services {
        if let Some(missing) = s
            .depends_on
            .iter()
            .find(|d| !index.contains_key(d.as_str()))
        {
            return Err(OrderError::UnknownDependency {
                service: s.name.clone(),
                missing: missing.clone(),
            });
        }
    }

    // Kahn's algorithm, always picking the lowest `order` among the ready ones.
    let mut remaining: Vec<usize> = (0..services.len()).collect();
    let mut done: HashSet<usize> = HashSet::new();
    let mut out = Vec::with_capacity(services.len());
    while !remaining.is_empty() {
        let mut ready: Vec<usize> = remaining
            .iter()
            .copied()
            .filter(|&i| {
                services[i]
                    .depends_on
                    .iter()
                    .all(|d| done.contains(&index[d.as_str()]))
            })
            .collect();
        if ready.is_empty() {
            let names: Vec<&str> = remaining
                .iter()
                .map(|&i| services[i].name.as_str())
                .collect();
            return Err(OrderError::Cycle(names.join(" → ")));
        }
        ready.sort_by(|&a, &b| by_order(&services[a], &services[b]));
        let next = ready[0];
        out.push(next);
        done.insert(next);
        remaining.retain(|&i| i != next);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectId;
    use crate::run::CommandId;
    use crate::workspace::model::ServiceSource;

    fn svc(id: &str, order: u32, deps: &[&str]) -> Service {
        Service {
            id: id.into(),
            name: id.into(),
            order,
            source: ServiceSource::Command {
                project_id: ProjectId("p".into()),
                command_id: CommandId(format!("script:{id}")),
                worktree: None,
            },
            port: None,
            health: None,
            depends_on: deps.iter().map(|d| (*d).into()).collect(),
        }
    }

    fn names(s: &[Service], order: &[usize]) -> Vec<String> {
        order.iter().map(|&i| s[i].id.clone()).collect()
    }

    #[test]
    fn sorts_by_order_then_name() {
        let s = [
            svc("web", 3, &[]),
            svc("db", 1, &[]),
            svc("cache", 2, &[]),
            svc("api", 2, &[]),
        ];
        assert_eq!(
            names(&s, &start_order(&s).unwrap()),
            vec!["db", "api", "cache", "web"]
        );
    }

    #[test]
    fn dependencies_win_over_order() {
        // web says order 1, but needs api, which needs db.
        let s = [
            svc("web", 1, &["api"]),
            svc("api", 5, &["db"]),
            svc("db", 9, &[]),
        ];
        assert_eq!(
            names(&s, &start_order(&s).unwrap()),
            vec!["db", "api", "web"]
        );
    }

    #[test]
    fn cycles_are_rejected() {
        let s = [svc("a", 1, &["b"]), svc("b", 2, &["a"]), svc("c", 3, &[])];
        assert!(matches!(start_order(&s), Err(OrderError::Cycle(_))));
    }

    #[test]
    fn unknown_dependency_is_rejected() {
        let s = [svc("a", 1, &["ghost"])];
        assert!(matches!(
            start_order(&s),
            Err(OrderError::UnknownDependency { .. })
        ));
    }

    #[test]
    fn empty_is_fine() {
        assert!(start_order(&[]).unwrap().is_empty());
    }
}
