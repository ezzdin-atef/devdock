//! Search, filter, sort and counts for the project list (T2.6, T2.7).
//! Pure functions over borrowed data: the result borrows from the input slice.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::detect::ProjectKind;
use crate::project::{Project, ProjectId};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Library {
    #[default]
    All,
    Favorites,
    Recent,
    Missing,
    /// Added by the most recent scan ("Review new").
    New,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    #[default]
    Recent,
    Name,
    Kind,
    /// The user's own order (drag and drop): the order projects are saved in.
    Manual,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Query {
    pub search: String,
    pub library: Library,
    pub kind: Option<ProjectKind>,
    pub sort: SortBy,
    /// Only this group's projects; `UNGROUPED` for projects in no group.
    pub group: Option<String>,
}

/// `Query::group` value for "projects in no group".
pub const UNGROUPED: &str = "-";

fn in_group(p: &Project, group: Option<&str>) -> bool {
    match group {
        None => true,
        Some(UNGROUPED) => p.group.is_none(),
        Some(g) => p.group.as_ref().is_some_and(|id| id.0 == g),
    }
}

/// Facts about projects that aren't stored on the project itself.
pub struct Flags<'a> {
    pub missing: &'a HashSet<ProjectId>,
    pub new: &'a HashSet<ProjectId>,
}

impl Flags<'_> {
    fn in_library(&self, p: &Project, library: Library) -> bool {
        match library {
            Library::All => true,
            Library::Favorites => p.favorite,
            Library::Recent => p.last_opened.is_some(),
            Library::Missing => self.missing.contains(&p.id),
            Library::New => self.new.contains(&p.id),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub all: usize,
    pub favorites: usize,
    pub recent: usize,
    pub missing: usize,
    pub new: usize,
    /// Per kind, within the current library + search (so chips match what you'd see).
    pub kinds: BTreeMap<ProjectKind, usize>,
    /// Projects per group id (all projects, like the library counts).
    pub groups: BTreeMap<String, usize>,
    pub ungrouped: usize,
}

fn matches_search(p: &Project, needle: &str) -> bool {
    needle.is_empty()
        || p.name.to_lowercase().contains(needle)
        || p.path.to_string_lossy().to_lowercase().contains(needle)
}

/// Projects matching `q`, sorted. Borrows from `projects`; nothing is cloned.
pub fn filter<'a>(projects: &'a [Project], q: &Query, flags: &Flags<'_>) -> Vec<&'a Project> {
    let needle = q.search.trim().to_lowercase();
    let mut out: Vec<&Project> = projects
        .iter()
        .filter(|p| flags.in_library(p, q.library))
        .filter(|p| matches_search(p, &needle))
        .filter(|p| q.kind.is_none_or(|k| p.kind == k))
        .filter(|p| in_group(p, q.group.as_deref()))
        .collect();

    let by_name = |a: &&Project, b: &&Project| a.name.to_lowercase().cmp(&b.name.to_lowercase());
    match q.sort {
        // `filter` keeps the saved order, which is the manual order.
        SortBy::Manual => {}
        SortBy::Name => out.sort_by(by_name),
        SortBy::Kind => out.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| by_name(a, b))),
        // Most recently opened first; never-opened ones after, newest added first.
        SortBy::Recent => out.sort_by(|a, b| {
            b.last_opened
                .cmp(&a.last_opened)
                .then_with(|| b.added_at.cmp(&a.added_at))
                .then_with(|| by_name(a, b))
        }),
    }
    out
}

pub fn counts(projects: &[Project], q: &Query, flags: &Flags<'_>) -> Counts {
    let needle = q.search.trim().to_lowercase();
    let count = |library| {
        projects
            .iter()
            .filter(|p| flags.in_library(p, library))
            .count()
    };
    let mut kinds = BTreeMap::new();
    for p in projects
        .iter()
        .filter(|p| flags.in_library(p, q.library) && matches_search(p, &needle))
    {
        *kinds.entry(p.kind).or_insert(0) += 1;
    }
    let mut groups = BTreeMap::new();
    for g in projects.iter().filter_map(|p| p.group.as_ref()) {
        *groups.entry(g.0.clone()).or_insert(0) += 1;
    }
    Counts {
        groups,
        ungrouped: projects.iter().filter(|p| p.group.is_none()).count(),
        all: projects.len(),
        favorites: count(Library::Favorites),
        recent: count(Library::Recent),
        missing: count(Library::Missing),
        new: count(Library::New),
        kinds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn p(name: &str, path: &str, kind: ProjectKind) -> Project {
        Project {
            id: ProjectId(name.into()),
            name: name.into(),
            path: PathBuf::from(path),
            kind,
            is_git: false,
            favorite: false,
            last_opened: None,
            added_at: 0,
            commands: Vec::new(),
            ports: Default::default(),
            run_prefs: Default::default(),
            domain: None,
            hide_port: None,
            group: None,
            launches: Vec::new(),
            activity: Vec::new(),
        }
    }

    fn sample() -> Vec<Project> {
        let mut shop = p("Shop", "D:\\dev\\shop", ProjectKind::NextJs);
        shop.favorite = true;
        shop.last_opened = Some(100);
        let mut api = p("api", "D:\\dev\\wallet\\api", ProjectKind::Rust);
        api.last_opened = Some(200);
        let mut docs = p("docs", "D:\\dev\\wallet\\docs", ProjectKind::Node);
        docs.added_at = 5;
        vec![shop, api, docs]
    }

    fn names(v: &[&Project]) -> Vec<String> {
        v.iter().map(|p| p.name.clone()).collect()
    }

    fn no_flags() -> (HashSet<ProjectId>, HashSet<ProjectId>) {
        (HashSet::new(), HashSet::new())
    }

    #[test]
    fn search_is_case_insensitive_on_name_and_path() {
        let all = sample();
        let (m, n) = no_flags();
        let flags = Flags {
            missing: &m,
            new: &n,
        };
        let q = Query {
            search: "WALLET".into(),
            sort: SortBy::Name,
            ..Query::default()
        };
        assert_eq!(names(&filter(&all, &q, &flags)), vec!["api", "docs"]);
        let q = Query {
            search: "shop".into(),
            ..Query::default()
        };
        assert_eq!(names(&filter(&all, &q, &flags)), vec!["Shop"]);
    }

    #[test]
    fn recent_sort_puts_last_opened_first_then_newest_added() {
        let all = sample();
        let (m, n) = no_flags();
        let flags = Flags {
            missing: &m,
            new: &n,
        };
        assert_eq!(
            names(&filter(&all, &Query::default(), &flags)),
            vec!["api", "Shop", "docs"]
        );
    }

    #[test]
    fn name_sort_ignores_case() {
        let all = sample();
        let (m, n) = no_flags();
        let flags = Flags {
            missing: &m,
            new: &n,
        };
        let q = Query {
            sort: SortBy::Name,
            ..Query::default()
        };
        assert_eq!(
            names(&filter(&all, &q, &flags)),
            vec!["api", "docs", "Shop"]
        );
    }

    #[test]
    fn kind_sort_follows_enum_order() {
        let all = sample();
        let (m, n) = no_flags();
        let flags = Flags {
            missing: &m,
            new: &n,
        };
        let q = Query {
            sort: SortBy::Kind,
            ..Query::default()
        };
        assert_eq!(
            names(&filter(&all, &q, &flags)),
            vec!["Shop", "docs", "api"]
        );
    }

    #[test]
    fn library_and_kind_filters() {
        let all = sample();
        let missing: HashSet<_> = [ProjectId("docs".into())].into();
        let new: HashSet<_> = [ProjectId("api".into())].into();
        let flags = Flags {
            missing: &missing,
            new: &new,
        };
        let by = |library| {
            names(&filter(
                &all,
                &Query {
                    library,
                    ..Query::default()
                },
                &flags,
            ))
        };
        assert_eq!(by(Library::Favorites), vec!["Shop"]);
        assert_eq!(by(Library::Recent), vec!["api", "Shop"]);
        assert_eq!(by(Library::Missing), vec!["docs"]);
        assert_eq!(by(Library::New), vec!["api"]);
        let q = Query {
            kind: Some(ProjectKind::Rust),
            ..Query::default()
        };
        assert_eq!(names(&filter(&all, &q, &flags)), vec!["api"]);
    }

    #[test]
    fn counts_libraries_over_everything_and_kinds_within_search() {
        let all = sample();
        let missing: HashSet<_> = [ProjectId("docs".into())].into();
        let (_, n) = no_flags();
        let flags = Flags {
            missing: &missing,
            new: &n,
        };
        let q = Query {
            search: "wallet".into(),
            ..Query::default()
        };
        let c = counts(&all, &q, &flags);
        assert_eq!(
            (c.all, c.favorites, c.recent, c.missing, c.new),
            (3, 1, 2, 1, 0)
        );
        assert_eq!(c.kinds.get(&ProjectKind::Rust), Some(&1));
        assert_eq!(c.kinds.get(&ProjectKind::Node), Some(&1));
        assert_eq!(c.kinds.get(&ProjectKind::NextJs), None);
    }

    #[test]
    fn query_deserializes_from_ui_shape() {
        let q: Query = serde_json::from_str(
            r#"{"search":"x","library":"favorites","kind":"nextJs","sort":"name"}"#,
        )
        .unwrap();
        assert_eq!(q.library, Library::Favorites);
        assert_eq!(q.kind, Some(ProjectKind::NextJs));
        let empty: Query = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, Query::default());
    }
}
