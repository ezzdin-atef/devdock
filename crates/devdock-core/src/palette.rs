//! Command palette (T8.2): items from several sources, fuzzy-matched, grouped.
//!
//! Each kind of thing (projects, commands, agents, workspaces, ports, actions) is a
//! [`PaletteSource`] that yields items through a boxed iterator, so the palette
//! doesn't care how many sources there are or how each builds its items.

use serde::Serialize;

use crate::agents::AgentId;
use crate::ports::{Owner, PortRow};
use crate::project::{Project, ProjectId};
use crate::run::{CommandId, RunCommand};
use crate::workspace::{Workspace, WorkspaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Projects,
    Commands,
    Agents,
    Workspaces,
    Ports,
    Actions,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Projects => "PROJECTS",
            Group::Commands => "COMMANDS",
            Group::Agents => "AGENTS",
            Group::Workspaces => "WORKSPACES",
            Group::Ports => "PORTS",
            Group::Actions => "ACTIONS",
        }
    }
}

/// What Enter does. The UI carries it out with its existing functions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PaletteAction {
    OpenProject {
        project_id: ProjectId,
    },
    RunCommand {
        project_id: ProjectId,
        command_id: CommandId,
    },
    LaunchAgent {
        project_id: ProjectId,
        agent: AgentId,
    },
    StartWorkspace {
        workspace_id: WorkspaceId,
    },
    StopWorkspace {
        workspace_id: WorkspaceId,
    },
    ShowPort {
        port: u16,
        pid: u32,
    },
    GoTo {
        view: String,
    },
    Rescan,
    StopEverything,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteItem {
    pub group: Group,
    /// What is shown and matched.
    pub label: String,
    /// Right-hand mono hint: shortcut, state or path.
    pub hint: String,
    /// Extra text that matches but isn't shown (a path, a command line).
    #[serde(skip)]
    pub keywords: String,
    pub action: PaletteAction,
}

pub trait PaletteSource {
    fn items(&self) -> Box<dyn Iterator<Item = PaletteItem> + '_>;
}

// ---------- matching ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub score: i32,
    /// Char indices in the label that matched (for bold).
    pub positions: Vec<usize>,
}

fn is_boundary(prev: Option<char>, cur: char) -> bool {
    match prev {
        None => true,
        Some(p) => {
            matches!(p, ' ' | '-' | '_' | '/' | '\\' | '.' | ':' | '·')
                || (p.is_lowercase() && cur.is_uppercase())
                || (!p.is_ascii_digit() && cur.is_ascii_digit())
        }
    }
}

/// Subsequence match with a best-score alignment (small DP; labels are short).
/// Word starts and runs of consecutive letters score high; every jump costs.
pub fn fuzzy(query: &str, text: &str) -> Option<Match> {
    let q: Vec<char> = query
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if q.is_empty() {
        return Some(Match {
            score: 0,
            positions: Vec::new(),
        });
    }
    let t: Vec<char> = text.chars().collect();
    let tl: Vec<char> = t
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let (n, m) = (q.len(), t.len());
    if n > m {
        return None;
    }
    const NONE: i32 = i32::MIN / 2;
    // best[i][j]: best score with q[i] matched at t[j]; from[i][j]: previous j.
    let mut best = vec![vec![NONE; m]; n];
    let mut from = vec![vec![usize::MAX; m]; n];
    for i in 0..n {
        for j in i..m {
            if tl[j] != q[i] {
                continue;
            }
            let bonus = if is_boundary(j.checked_sub(1).map(|k| t[k]), t[j]) {
                if j == 0 { 12 } else { 8 }
            } else {
                1
            };
            if i == 0 {
                best[i][j] = bonus - j as i32 / 4;
                continue;
            }
            for k in (i - 1)..j {
                if best[i - 1][k] == NONE {
                    continue;
                }
                let gap = (j - k - 1) as i32;
                // A run of letters earns +10; any jump costs 3 plus its length (max 10).
                let s = best[i - 1][k] + bonus + if gap == 0 { 10 } else { -(3 + gap.min(10)) };
                if s > best[i][j] {
                    best[i][j] = s;
                    from[i][j] = k;
                }
            }
        }
    }
    let (end, score) = (0..m)
        .filter(|&j| best[n - 1][j] != NONE)
        .map(|j| (j, best[n - 1][j]))
        .max_by_key(|&(j, s)| (s, std::cmp::Reverse(j)))?;
    let mut positions = vec![end];
    let mut j = end;
    for i in (1..n).rev() {
        j = from[i][j];
        positions.push(j);
    }
    positions.reverse();
    Some(Match {
        score: score - (m as i32) / 8,
        positions,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    #[serde(flatten)]
    pub item: PaletteItem,
    pub positions: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultGroup {
    pub group: Group,
    pub title: &'static str,
    pub hits: Vec<Hit>,
}

/// Best `per_group` hits per group, groups in a fixed order. An empty query lists
/// items in source order (projects come most-recent first).
pub fn search(items: &[PaletteItem], query: &str, per_group: usize) -> Vec<ResultGroup> {
    let mut scored: Vec<(i32, &PaletteItem, Vec<usize>)> = items
        .iter()
        .filter_map(|it| {
            if let Some(m) = fuzzy(query, &it.label) {
                return Some((m.score, it, m.positions));
            }
            // Matching only the hidden keywords still counts, without highlights.
            fuzzy(query, &it.keywords).map(|m| (m.score - 20, it, Vec::new()))
        })
        .collect();
    if !query.trim().is_empty() {
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.label.len().cmp(&b.1.label.len())));
    }
    let mut groups: Vec<ResultGroup> = Vec::new();
    for (_, item, positions) in scored {
        let g = match groups.iter_mut().find(|g| g.group == item.group) {
            Some(g) => g,
            None => {
                groups.push(ResultGroup {
                    group: item.group,
                    title: item.group.title(),
                    hits: Vec::new(),
                });
                groups.last_mut().unwrap_or_else(|| unreachable!())
            }
        };
        if g.hits.len() < per_group {
            g.hits.push(Hit {
                item: item.clone(),
                positions,
            });
        }
    }
    groups.sort_by_key(|g| g.group);
    groups
}

// ---------- sources ----------

pub struct ProjectsSource<'a> {
    pub projects: &'a [Project],
}

impl PaletteSource for ProjectsSource<'_> {
    fn items(&self) -> Box<dyn Iterator<Item = PaletteItem> + '_> {
        let mut sorted: Vec<&Project> = self.projects.iter().collect();
        sorted.sort_by(|a, b| b.last_opened.cmp(&a.last_opened).then(a.name.cmp(&b.name)));
        Box::new(sorted.into_iter().map(|p| PaletteItem {
            group: Group::Projects,
            label: p.name.clone(),
            hint: p.kind.label().into(),
            keywords: p.path.to_string_lossy().into_owned(),
            action: PaletteAction::OpenProject {
                project_id: p.id.clone(),
            },
        }))
    }
}

pub struct CommandsSource<'a> {
    pub entries: &'a [(Project, Vec<RunCommand>)],
}

impl PaletteSource for CommandsSource<'_> {
    fn items(&self) -> Box<dyn Iterator<Item = PaletteItem> + '_> {
        Box::new(self.entries.iter().flat_map(|(p, cmds)| {
            cmds.iter().map(move |c| PaletteItem {
                group: Group::Commands,
                label: format!("Run {} in {}", c.name, p.name),
                hint: c.command_line.clone(),
                keywords: c.command_line.clone(),
                action: PaletteAction::RunCommand {
                    project_id: p.id.clone(),
                    command_id: c.id.clone(),
                },
            })
        }))
    }
}

pub struct AgentsSource<'a> {
    pub projects: &'a [Project],
    /// Installed agents only.
    pub agents: &'a [AgentId],
}

impl PaletteSource for AgentsSource<'_> {
    fn items(&self) -> Box<dyn Iterator<Item = PaletteItem> + '_> {
        Box::new(
            self.projects
                .iter()
                .filter(|p| p.path.is_dir())
                .flat_map(|p| {
                    self.agents.iter().map(move |a| PaletteItem {
                        group: Group::Agents,
                        label: format!("{} in {}", a.name(), p.name),
                        hint: a.command().into(),
                        keywords: String::new(),
                        action: PaletteAction::LaunchAgent {
                            project_id: p.id.clone(),
                            agent: *a,
                        },
                    })
                }),
        )
    }
}

pub struct WorkspacesSource<'a> {
    pub workspaces: &'a [Workspace],
    /// Ids of workspaces that are starting or running.
    pub live: &'a [WorkspaceId],
}

impl PaletteSource for WorkspacesSource<'_> {
    fn items(&self) -> Box<dyn Iterator<Item = PaletteItem> + '_> {
        Box::new(self.workspaces.iter().map(|w| {
            let running = self.live.contains(&w.id);
            PaletteItem {
                group: Group::Workspaces,
                label: format!("{} {}", if running { "Stop" } else { "Start" }, w.name),
                hint: if running {
                    "running".into()
                } else {
                    format!("{} services", w.services.len())
                },
                keywords: w.name.clone(),
                action: if running {
                    PaletteAction::StopWorkspace {
                        workspace_id: w.id.clone(),
                    }
                } else {
                    PaletteAction::StartWorkspace {
                        workspace_id: w.id.clone(),
                    }
                },
            }
        }))
    }
}

pub struct PortsSource<'a> {
    pub rows: &'a [PortRow],
}

impl PaletteSource for PortsSource<'_> {
    fn items(&self) -> Box<dyn Iterator<Item = PaletteItem> + '_> {
        Box::new(
            self.rows
                .iter()
                .filter(|r| r.owner != Owner::System)
                .map(|r| {
                    let name = r.process.as_ref().map_or("unknown", |p| p.name.as_str());
                    PaletteItem {
                        group: Group::Ports,
                        label: format!("Port {} · {name}", r.port),
                        hint: match &r.project {
                            Some(p) => p.name.clone(),
                            None => format!("PID {}", r.pid),
                        },
                        keywords: format!("who is using port {} {}", r.port, r.pid),
                        action: PaletteAction::ShowPort {
                            port: r.port,
                            pid: r.pid,
                        },
                    }
                }),
        )
    }
}

pub struct ActionsSource;

impl PaletteSource for ActionsSource {
    fn items(&self) -> Box<dyn Iterator<Item = PaletteItem> + '_> {
        let go = |label: &str, view: &str| PaletteItem {
            group: Group::Actions,
            label: label.into(),
            hint: String::new(),
            keywords: String::new(),
            action: PaletteAction::GoTo { view: view.into() },
        };
        Box::new(
            [
                go("Go to Projects", "projects"),
                go("Go to Spaces", "spaces"),
                go("Go to Running", "running"),
                go("Go to Ports", "ports"),
                go("Go to Agents", "agents"),
                go("Go to Settings", "settings"),
                PaletteItem {
                    group: Group::Actions,
                    label: "Rescan all scan roots".into(),
                    hint: String::new(),
                    keywords: String::new(),
                    action: PaletteAction::Rescan,
                },
                PaletteItem {
                    group: Group::Actions,
                    label: "Stop everything DevDock is running".into(),
                    hint: String::new(),
                    keywords: "kill all".into(),
                    action: PaletteAction::StopEverything,
                },
            ]
            .into_iter(),
        )
    }
}

/// Collects every source into one list.
pub fn collect(sources: &[&dyn PaletteSource]) -> Vec<PaletteItem> {
    sources.iter().flat_map(|s| s.items()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(group: Group, label: &str) -> PaletteItem {
        PaletteItem {
            group,
            label: label.into(),
            hint: String::new(),
            keywords: String::new(),
            action: PaletteAction::Rescan,
        }
    }

    #[test]
    fn subsequence_required() {
        assert!(fuzzy("chr", "devchrono").is_some());
        assert!(fuzzy("rhc", "devchrono").is_none());
        assert!(fuzzy("devchronox", "devchrono").is_none());
    }

    #[test]
    fn positions_prefer_word_starts_and_runs() {
        let text = "Run dev in DevChrono";
        let m = fuzzy("rdc", text).unwrap();
        let label: Vec<char> = text.chars().collect();
        let picked: String = m.positions.iter().map(|&i| label[i]).collect();
        assert_eq!(picked.to_lowercase(), "rdc");
        // Every matched letter starts a word (R-un, d-ev or D-ev, C-hrono).
        for &i in &m.positions {
            assert!(
                is_boundary(i.checked_sub(1).map(|k| label[k]), label[i]),
                "{i} in {:?}",
                m.positions
            );
        }
    }

    #[test]
    fn better_matches_rank_higher() {
        let start = fuzzy("chrono", "devchrono").unwrap().score;
        let word = fuzzy("chrono", "Chrono API").unwrap().score;
        let scattered = fuzzy("chrono", "cache helper runs on node")
            .map(|m| m.score)
            .unwrap_or(i32::MIN);
        assert!(word > start, "{word} vs {start}");
        assert!(start > scattered, "{start} vs {scattered}");
    }

    #[test]
    fn search_groups_and_limits() {
        let items = vec![
            item(Group::Projects, "devchrono"),
            item(Group::Projects, "shop"),
            item(Group::Commands, "Run dev in devchrono"),
            item(Group::Commands, "Run build in devchrono"),
            item(Group::Actions, "Go to Projects"),
        ];
        let r = search(&items, "chrono", 1);
        assert_eq!(
            r.iter().map(|g| g.title).collect::<Vec<_>>(),
            vec!["PROJECTS", "COMMANDS"]
        );
        assert_eq!(r[1].hits.len(), 1, "limited per group");
        assert_eq!(r[0].hits[0].item.label, "devchrono");
    }

    #[test]
    fn empty_query_keeps_source_order() {
        let items = vec![item(Group::Projects, "b"), item(Group::Projects, "a")];
        let r = search(&items, "  ", 5);
        assert_eq!(
            r[0].hits
                .iter()
                .map(|h| h.item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a"]
        );
    }

    #[test]
    fn hidden_keywords_match_without_highlights() {
        let mut it = item(Group::Ports, "Port 8069 · python.exe");
        it.keywords = "who is using port 8069 9120".into();
        let r = search(&[it], "9120", 5);
        assert_eq!(r[0].hits.len(), 1);
        assert!(r[0].hits[0].positions.is_empty());
    }

    #[test]
    fn sources_produce_actions() {
        let items = collect(&[&ActionsSource]);
        assert!(
            items
                .iter()
                .any(|i| i.action == PaletteAction::StopEverything)
        );
        assert!(items.iter().all(|i| i.group == Group::Actions));
    }
}
