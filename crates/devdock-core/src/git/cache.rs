//! Session cache of git results per project, plus which refreshes are in flight
//! so the same project isn't queried twice at once (T3.5).

use std::collections::{HashMap, HashSet};

use super::{GitStatus, RepoInfo, Snapshot};
use crate::project::ProjectId;

/// What to fetch: the cheap status for the list, or full details for the Git tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Depth {
    Status,
    Details,
}

#[derive(Debug, Default)]
pub struct GitCache {
    status: HashMap<ProjectId, Snapshot<GitStatus>>,
    details: HashMap<ProjectId, Snapshot<RepoInfo>>,
    in_flight: HashSet<(ProjectId, Depth)>,
}

impl GitCache {
    pub fn status(&self, id: &ProjectId) -> Option<&Snapshot<GitStatus>> {
        self.status.get(id)
    }

    pub fn details(&self, id: &ProjectId) -> Option<&Snapshot<RepoInfo>> {
        self.details.get(id)
    }

    /// Marks `id` as being refreshed. Returns false if that refresh is already running.
    pub fn claim(&mut self, id: &ProjectId, depth: Depth) -> bool {
        self.in_flight.insert((id.clone(), depth))
    }

    pub fn store_status(&mut self, id: &ProjectId, snap: Snapshot<GitStatus>) {
        self.in_flight.remove(&(id.clone(), Depth::Status));
        self.status.insert(id.clone(), snap);
    }

    /// Details include a fresh status, so the list badge updates too.
    pub fn store_details(&mut self, id: &ProjectId, snap: Snapshot<RepoInfo>) {
        self.in_flight.remove(&(id.clone(), Depth::Details));
        self.status
            .insert(id.clone(), snap.map(|info| info.status.clone()));
        self.details.insert(id.clone(), snap);
    }

    /// Releases a claim without a result (the folder vanished, say).
    pub fn release(&mut self, id: &ProjectId, depth: Depth) {
        self.in_flight.remove(&(id.clone(), depth));
    }

    pub fn forget(&mut self, id: &ProjectId) {
        self.status.remove(id);
        self.details.remove(id);
        self.in_flight.retain(|(i, _)| i != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_are_exclusive_per_depth() {
        let mut c = GitCache::default();
        let id = ProjectId("a".into());
        assert!(c.claim(&id, Depth::Status));
        assert!(!c.claim(&id, Depth::Status));
        assert!(c.claim(&id, Depth::Details));
        c.store_status(&id, Snapshot::NoRepo { refreshed_at: 1 });
        assert!(c.claim(&id, Depth::Status));
    }

    #[test]
    fn forget_drops_everything() {
        let mut c = GitCache::default();
        let id = ProjectId("a".into());
        c.claim(&id, Depth::Details);
        c.store_status(&id, Snapshot::NoRepo { refreshed_at: 1 });
        c.forget(&id);
        assert!(c.status(&id).is_none());
        assert!(c.claim(&id, Depth::Details));
    }
}
