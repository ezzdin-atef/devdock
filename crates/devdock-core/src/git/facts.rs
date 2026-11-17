//! Repository facts beyond status (T3.3, T3.4): root, worktree detection, last commit,
//! remotes and worktrees. Each parser is pure; `repo_info` runs the commands.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use super::GitError;
use super::run::{DEFAULT_TIMEOUT, git};
use super::status::{self, GitStatus};
use crate::paths;

const QUICK: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastCommit {
    pub hash: String,
    pub subject: String,
    pub author: String,
    /// Git's own wording, e.g. "3 hours ago".
    pub relative: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Remote {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub detached: bool,
    pub bare: bool,
    /// The worktree this project lives in.
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub root: PathBuf,
    /// The project is a linked worktree (`git worktree add`), not the main checkout.
    pub is_worktree: bool,
    /// The project folder is below the repo root, e.g. one app in a monorepo.
    pub in_subfolder: bool,
    pub status: GitStatus,
    pub last_commit: Option<LastCommit>,
    pub remotes: Vec<Remote>,
    pub worktrees: Vec<Worktree>,
}

/// Just the status: what the project list needs. One git call.
pub fn status(dir: &Path) -> Result<GitStatus, GitError> {
    let out = git(
        dir,
        &["status", "--porcelain=v2", "--branch"],
        DEFAULT_TIMEOUT,
    )?;
    Ok(status::parse(&out))
}

/// Everything the Git tab shows. Five git calls; run off the UI thread.
pub fn repo_info(dir: &Path) -> Result<RepoInfo, GitError> {
    let locations = git(
        dir,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-dir",
            "--git-common-dir",
        ],
        QUICK,
    )?;
    let (root, git_dir, common_dir) = parse_locations(&locations)?;
    let status = status(dir)?;

    // `git log` fails in a repo with no commits yet; that's "no last commit", not an error.
    let last_commit = if status.oid.is_some() {
        git(
            dir,
            &["log", "-1", "--format=%h%x00%s%x00%an%x00%cr"],
            QUICK,
        )
        .ok()
        .and_then(|out| parse_last_commit(&out))
    } else {
        None
    };
    let remotes = parse_remotes(&git(dir, &["remote", "-v"], QUICK)?);
    let worktrees = parse_worktrees(
        &git(dir, &["worktree", "list", "--porcelain"], QUICK)?,
        &root,
    );

    Ok(RepoInfo {
        in_subfolder: paths::key(dir) != paths::key(&root),
        is_worktree: paths::key(&git_dir) != paths::key(&common_dir),
        root,
        status,
        last_commit,
        remotes,
        worktrees,
    })
}

fn parse_locations(out: &str) -> Result<(PathBuf, PathBuf, PathBuf), GitError> {
    let mut lines = out.lines().map(str::trim).filter(|l| !l.is_empty());
    match (lines.next(), lines.next(), lines.next()) {
        (Some(root), Some(git_dir), Some(common)) => Ok((
            paths::normalize(Path::new(root)),
            paths::normalize(Path::new(git_dir)),
            paths::normalize(Path::new(common)),
        )),
        // A bare repo has no work tree, so --show-toplevel prints nothing.
        _ => Err(GitError::Failed(
            "this is a bare repository with no working folder".into(),
        )),
    }
}

pub fn parse_last_commit(out: &str) -> Option<LastCommit> {
    let mut parts = out.trim_end_matches(['\r', '\n']).split('\0');
    Some(LastCommit {
        hash: parts.next().filter(|h| !h.is_empty())?.to_owned(),
        subject: parts.next()?.to_owned(),
        author: parts.next()?.to_owned(),
        relative: parts.next()?.to_owned(),
    })
}

/// `origin\thttps://x (fetch)` lines; one entry per remote, preferring the fetch URL.
pub fn parse_remotes(out: &str) -> Vec<Remote> {
    let mut remotes: Vec<Remote> = Vec::new();
    for line in out.lines() {
        let Some((name, rest)) = line.split_once('\t') else {
            continue;
        };
        let url = rest
            .trim_end_matches(" (fetch)")
            .trim_end_matches(" (push)")
            .to_owned();
        if !remotes.iter().any(|r| r.name == name) {
            remotes.push(Remote {
                name: name.to_owned(),
                url,
            });
        }
    }
    remotes
}

/// Blocks separated by blank lines: `worktree <path>`, `HEAD <sha>`,
/// `branch refs/heads/<name>`, `detached`, `bare`.
pub fn parse_worktrees(out: &str, current_root: &Path) -> Vec<Worktree> {
    let current = paths::key(current_root);
    let mut list = Vec::new();
    let mut cur: Option<Worktree> = None;

    for line in out.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            list.extend(cur.take());
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            list.extend(cur.take());
            let path = paths::normalize(Path::new(path));
            cur = Some(Worktree {
                is_current: paths::key(&path) == current,
                path,
                branch: None,
                detached: false,
                bare: false,
            });
        } else if let Some(wt) = cur.as_mut() {
            if let Some(b) = line.strip_prefix("branch ") {
                wt.branch = Some(b.trim_start_matches("refs/heads/").to_owned());
            } else if line == "detached" {
                wt.detached = true;
            } else if line == "bare" {
                wt.bare = true;
            }
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_commit_splits_on_nul() {
        let c = parse_last_commit("a1b2c3d\0Fix login: handle 401\0Ada Lovelace\x003 hours ago\n")
            .unwrap();
        assert_eq!(c.hash, "a1b2c3d");
        assert_eq!(c.subject, "Fix login: handle 401");
        assert_eq!(c.author, "Ada Lovelace");
        assert_eq!(c.relative, "3 hours ago");
        assert_eq!(parse_last_commit(""), None);
    }

    #[test]
    fn remotes_are_deduplicated() {
        let out = "origin\thttps://github.com/me/x.git (fetch)\norigin\thttps://github.com/me/x.git (push)\nupstream\tgit@github.com:org/x.git (fetch)\nupstream\tgit@github.com:org/x.git (push)\n";
        let r = parse_remotes(out);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].name, "upstream");
        assert_eq!(r[1].url, "git@github.com:org/x.git");
    }

    #[test]
    fn worktrees_parse_branch_detached_and_current() {
        let out = "worktree Q:/dev/shop\nHEAD 1111111111111111111111111111111111111111\nbranch refs/heads/main\n\nworktree Q:/dev/shop-hotfix\nHEAD 2222222222222222222222222222222222222222\ndetached\n\n";
        let w = parse_worktrees(out, Path::new("Q:\\dev\\shop-hotfix"));
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].branch.as_deref(), Some("main"));
        assert!(!w[0].is_current);
        assert!(w[1].detached && w[1].is_current);
        assert_eq!(w[1].path, PathBuf::from("Q:\\dev\\shop-hotfix"));
    }

    #[test]
    fn locations_need_three_lines() {
        assert!(parse_locations("C:/a\nC:/a/.git\n").is_err());
        let (root, gd, cd) = parse_locations("Q:/r\nQ:/r/.git/worktrees/x\nQ:/r/.git\n").unwrap();
        assert_eq!(root, PathBuf::from("Q:\\r"));
        assert_ne!(paths::key(&gd), paths::key(&cd));
    }
}
