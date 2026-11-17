//! Parsing `git status --porcelain=v2 --branch` (T3.2).
//!
//! Format reference (git-status(1), "Porcelain Format Version 2"):
//! ```text
//! # branch.oid <commit> | (initial)
//! # branch.head <branch> | (detached)
//! # branch.upstream <upstream>
//! # branch.ab +<ahead> -<behind>
//! 1 <XY> ...            changed entry
//! 2 <XY> ... <path>\t<orig>   renamed or copied
//! u <XY> ...            unmerged (conflict)
//! ? <path>              untracked
//! ```
//! `X` is the staged state and `Y` the worktree state; `.` means unchanged.

use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    /// Current commit, or `None` before the first commit.
    pub oid: Option<String>,
    /// Branch name, or `None` when HEAD is detached.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    /// Only known when there is an upstream.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    pub conflicted: u32,
    /// Files with any change. A file that is staged and also modified counts once,
    /// so this is not the sum of the counters. Shown as "N changes" in the list.
    pub changed_files: u32,
}

impl GitStatus {
    pub fn is_detached(&self) -> bool {
        self.branch.is_none()
    }
}

pub fn parse(output: &str) -> GitStatus {
    let mut s = GitStatus::default();

    for line in output.lines() {
        if let Some(header) = line.strip_prefix("# ") {
            let (key, value) = header.split_once(' ').unwrap_or((header, ""));
            match key {
                "branch.oid" => s.oid = (value != "(initial)").then(|| value.to_owned()),
                "branch.head" => s.branch = (value != "(detached)").then(|| value.to_owned()),
                "branch.upstream" => s.upstream = Some(value.to_owned()),
                "branch.ab" => {
                    let mut parts = value.split_whitespace();
                    s.ahead = parts
                        .next()
                        .and_then(|a| a.trim_start_matches('+').parse().ok());
                    s.behind = parts
                        .next()
                        .and_then(|b| b.trim_start_matches('-').parse().ok());
                }
                _ => {}
            }
            continue;
        }

        let mut fields = line.splitn(3, ' ');
        let kind = fields.next().unwrap_or("");
        let xy = fields.next().unwrap_or("");
        match kind {
            "1" | "2" => {
                let mut chars = xy.chars();
                let x = chars.next().unwrap_or('.');
                let y = chars.next().unwrap_or('.');
                if x != '.' {
                    s.staged += 1;
                }
                if y != '.' {
                    s.modified += 1;
                }
                s.changed_files += 1;
            }
            "u" => {
                s.conflicted += 1;
                s.changed_files += 1;
            }
            "?" => {
                s.untracked += 1;
                s.changed_files += 1;
            }
            _ => {} // "!" ignored files and anything unknown
        }
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN: &str = "\
# branch.oid 1f2e3d4c5b6a79881f2e3d4c5b6a79881f2e3d4c
# branch.head main
# branch.upstream origin/main
# branch.ab +0 -0
";

    const DIRTY: &str = "\
# branch.oid 1f2e3d4c5b6a79881f2e3d4c5b6a79881f2e3d4c
# branch.head feature/login
# branch.upstream origin/feature/login
# branch.ab +2 -1
1 M. N... 100644 100644 100644 aaaaaaa bbbbbbb src/a.ts
1 .M N... 100644 100644 100644 aaaaaaa aaaaaaa src/b.ts
1 MM N... 100644 100644 100644 aaaaaaa bbbbbbb src/c.ts
1 A. N... 000000 100644 100644 0000000 bbbbbbb src/new.ts
1 .D N... 100644 100644 000000 aaaaaaa aaaaaaa src/gone.ts
? notes.md
? scratch/
";

    const DETACHED: &str = "\
# branch.oid 1f2e3d4c5b6a79881f2e3d4c5b6a79881f2e3d4c
# branch.head (detached)
1 .M N... 100644 100644 100644 aaaaaaa aaaaaaa README.md
";

    const NO_UPSTREAM: &str = "\
# branch.oid 1f2e3d4c5b6a79881f2e3d4c5b6a79881f2e3d4c
# branch.head spike
";

    const RENAMED: &str = "\
# branch.oid 1f2e3d4c5b6a79881f2e3d4c5b6a79881f2e3d4c
# branch.head main
2 R. N... 100644 100644 100644 aaaaaaa aaaaaaa R100 src/new name.ts\tsrc/old name.ts
2 RM N... 100644 100644 100644 aaaaaaa bbbbbbb R087 lib/x.rs\tlib/y.rs
";

    const INITIAL: &str = "\
# branch.oid (initial)
# branch.head main
? README.md
";

    const CONFLICT: &str = "\
# branch.oid 1f2e3d4c5b6a79881f2e3d4c5b6a79881f2e3d4c
# branch.head main
u UU N... 100644 100644 100644 100644 aaaaaaa bbbbbbb ccccccc src/both.ts
1 M. N... 100644 100644 100644 aaaaaaa bbbbbbb src/ok.ts
";

    #[test]
    fn clean_with_upstream() {
        let p = parse(CLEAN);
        assert_eq!(p.branch.as_deref(), Some("main"));
        assert_eq!(p.upstream.as_deref(), Some("origin/main"));
        assert_eq!((p.ahead, p.behind), (Some(0), Some(0)));
        assert_eq!(
            (p.staged, p.modified, p.untracked, p.changed_files),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn dirty_counts_staged_and_modified_separately() {
        let p = parse(DIRTY);
        assert_eq!(p.branch.as_deref(), Some("feature/login"));
        assert_eq!((p.ahead, p.behind), (Some(2), Some(1)));
        // a(M.) c(MM) new(A.) are staged; b(.M) c(MM) gone(.D) are modified.
        assert_eq!((p.staged, p.modified, p.untracked), (3, 3, 2));
        assert_eq!(p.changed_files, 7);
    }

    #[test]
    fn detached_head() {
        let p = parse(DETACHED);
        assert!(p.is_detached());
        assert_eq!(p.upstream, None);
        assert_eq!(p.modified, 1);
    }

    #[test]
    fn no_upstream_means_unknown_ahead_behind() {
        let p = parse(NO_UPSTREAM);
        assert_eq!(p.branch.as_deref(), Some("spike"));
        assert_eq!((p.upstream.as_ref(), p.ahead, p.behind), (None, None, None));
    }

    #[test]
    fn renamed_files_with_spaces_count_once() {
        let p = parse(RENAMED);
        assert_eq!((p.staged, p.modified, p.changed_files), (2, 1, 2));
    }

    #[test]
    fn before_first_commit() {
        let p = parse(INITIAL);
        assert_eq!(p.oid, None);
        assert_eq!(p.branch.as_deref(), Some("main"));
        assert_eq!(p.untracked, 1);
    }

    #[test]
    fn conflicts_are_counted_apart() {
        let p = parse(CONFLICT);
        assert_eq!((p.conflicted, p.staged, p.changed_files), (1, 1, 2));
    }

    #[test]
    fn serializes_camel_case() {
        let v = serde_json::to_value(parse(DIRTY)).unwrap();
        assert_eq!(v["changedFiles"], 7);
        assert_eq!(v["branch"], "feature/login");
    }
}
