//! Changing a repository from the Git tab: list branches, switch, create, fetch,
//! pull and push. Everything else in `git` only reads; these are the only writes,
//! each one a plain git command the user asked for, with git's own output shown.
//!
//! Pull is fast-forward only: DevDock never makes a merge or rebase on its own.
//! When the branch and its upstream have both moved, git says so and the user
//! decides in a terminal.

use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::{GitError, git};

/// Network operations can be slow (big fetches, slow remotes).
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(300);
const LOCAL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalBranch {
    pub name: String,
    pub current: bool,
    /// e.g. `origin/main`.
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// The upstream branch was deleted on the remote.
    pub gone: bool,
    /// "3 days ago".
    pub updated: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branches {
    pub local: Vec<LocalBranch>,
    /// Remote branches with no local branch of the same name (`origin/feature`).
    pub remote_only: Vec<String>,
}

/// One action from the Git tab.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum GitOp {
    Fetch,
    Pull,
    Push,
    /// A local branch, or a remote one (`origin/x`), which gets a local tracking branch.
    Switch {
        branch: String,
    },
    /// A new branch from the current commit, switched to.
    Create {
        name: String,
    },
}

/// What git printed, for the result panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    /// Short sentence for the panel title: "Pulled 3 commits", "Switched to dev".
    pub summary: String,
    /// git's own output (stdout and stderr), trimmed.
    pub output: String,
}

pub fn branches(dir: &Path) -> Result<Branches, GitError> {
    let fmt = "%(HEAD)%09%(refname:short)%09%(upstream:short)%09%(upstream:track,nobracket)%09%(committerdate:relative)";
    let out = git(
        dir,
        &[
            "for-each-ref",
            &format!("--format={fmt}"),
            "--sort=-committerdate",
            "refs/heads",
        ],
        LOCAL_TIMEOUT,
    )?;
    let local: Vec<LocalBranch> = out.lines().filter_map(parse_local).collect();
    let remotes = git(
        dir,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "--sort=-committerdate",
            "refs/remotes",
        ],
        LOCAL_TIMEOUT,
    )?;
    let remote_only = remotes
        .lines()
        .map(str::trim)
        // `origin/HEAD` shows as just `origin`.
        .filter(|r| r.contains('/') && !r.ends_with("/HEAD"))
        .filter(|r| {
            let short = r.split_once('/').map_or(*r, |(_, b)| b);
            !local.iter().any(|l| l.name == short)
        })
        .map(str::to_owned)
        .collect();
    Ok(Branches { local, remote_only })
}

fn parse_local(line: &str) -> Option<LocalBranch> {
    let mut f = line.split('\t');
    let head = f.next()?;
    let name = f.next()?.trim();
    if name.is_empty() {
        return None;
    }
    let upstream = f.next().map(str::trim).filter(|s| !s.is_empty());
    let track = f.next().unwrap_or("").trim();
    let updated = f.next().unwrap_or("").trim().to_owned();
    let count = |word: &str| {
        track
            .split(',')
            .map(str::trim)
            .find_map(|part| part.strip_prefix(word))
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or(0)
    };
    Some(LocalBranch {
        name: name.to_owned(),
        current: head.trim() == "*",
        upstream: upstream.map(str::to_owned),
        ahead: count("ahead "),
        behind: count("behind "),
        gone: track == "gone",
        updated,
    })
}

/// Runs one action. Errors that are git's answer (a conflict, a rejected push) come
/// back as `ok: false` with git's words; `Err` is for git missing or hanging.
pub fn run(dir: &Path, op: &GitOp) -> Result<OpResult, GitError> {
    match op {
        GitOp::Fetch => {
            let r = exec(dir, &["fetch", "--all", "--prune"], NETWORK_TIMEOUT)?;
            Ok(r.into_result("Fetched from every remote", "Fetch failed"))
        }
        GitOp::Pull => {
            let before = head(dir);
            let r = exec(dir, &["pull", "--ff-only"], NETWORK_TIMEOUT)?;
            if !r.ok {
                let diverged = r.text.contains("Not possible to fast-forward")
                    || r.text.contains("diverging")
                    || r.text.contains("divergent");
                let title = if diverged {
                    "Your branch and its upstream have both changed. Merge or rebase in a terminal."
                } else {
                    "Pull failed"
                };
                return Ok(r.into_result("", title));
            }
            let pulled = match (before, head(dir)) {
                (Some(a), Some(b)) if a != b => commits_between(dir, &a, &b),
                _ => 0,
            };
            let summary = match pulled {
                0 => "Already up to date".to_owned(),
                1 => "Pulled 1 commit".to_owned(),
                n => format!("Pulled {n} commits"),
            };
            Ok(r.into_result(&summary, ""))
        }
        GitOp::Push => {
            let r = exec(dir, &["push"], NETWORK_TIMEOUT)?;
            if r.ok {
                return Ok(r.into_result("Pushed", ""));
            }
            // First push of a new branch: set its upstream on the first remote.
            if r.text.contains("has no upstream branch") {
                if let Some(remote) = first_remote(dir) {
                    let r = exec(dir, &["push", "-u", &remote, "HEAD"], NETWORK_TIMEOUT)?;
                    return Ok(r.into_result(
                        &format!("Pushed and set the upstream to {remote}"),
                        "Push failed",
                    ));
                }
                return Ok(r.into_result("", "This repository has no remote to push to"));
            }
            let rejected = r.text.contains("[rejected]") || r.text.contains("fetch first");
            Ok(r.into_result(
                "",
                if rejected {
                    "The remote has commits you don't have. Pull first."
                } else {
                    "Push failed"
                },
            ))
        }
        GitOp::Switch { branch } => {
            check_name(dir, branch.split_once('/').map_or(branch, |(_, b)| b))?;
            let r = if is_remote_branch(dir, branch) {
                exec(dir, &["switch", "--track", branch], LOCAL_TIMEOUT)?
            } else {
                exec(dir, &["switch", branch], LOCAL_TIMEOUT)?
            };
            let local = if is_remote_branch(dir, branch) {
                branch.split_once('/').map_or(branch.as_str(), |(_, b)| b)
            } else {
                branch.as_str()
            };
            let title = if r.text.contains("would be overwritten") {
                "Your changes would be overwritten. Commit or stash them first."
            } else {
                "Couldn't switch branches"
            };
            Ok(r.into_result(&format!("Switched to {local}"), title))
        }
        GitOp::Create { name } => {
            let name = name.trim();
            check_name(dir, name)?;
            let r = exec(dir, &["switch", "-c", name], LOCAL_TIMEOUT)?;
            Ok(r.into_result(
                &format!("Created and switched to {name}"),
                "Couldn't create the branch",
            ))
        }
    }
}

/// `git check-ref-format --branch`: the rules git itself uses.
fn check_name(dir: &Path, name: &str) -> Result<(), GitError> {
    if name.is_empty() || name.starts_with('-') {
        return Err(GitError::Failed("Type a branch name.".into()));
    }
    git(dir, &["check-ref-format", "--branch", name], LOCAL_TIMEOUT)
        .map(drop)
        .map_err(|_| GitError::Failed(format!("\"{name}\" isn't a valid branch name.")))
}

fn is_remote_branch(dir: &Path, name: &str) -> bool {
    git(
        dir,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/remotes/{name}"),
        ],
        LOCAL_TIMEOUT,
    )
    .is_ok()
        && git(
            dir,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{name}"),
            ],
            LOCAL_TIMEOUT,
        )
        .is_err()
}

fn head(dir: &Path) -> Option<String> {
    git(dir, &["rev-parse", "HEAD"], LOCAL_TIMEOUT)
        .ok()
        .map(|s| s.trim().to_owned())
}

fn commits_between(dir: &Path, a: &str, b: &str) -> usize {
    git(
        dir,
        &["rev-list", "--count", &format!("{a}..{b}")],
        LOCAL_TIMEOUT,
    )
    .ok()
    .and_then(|s| s.trim().parse().ok())
    .unwrap_or(0)
}

fn first_remote(dir: &Path) -> Option<String> {
    git(dir, &["remote"], LOCAL_TIMEOUT)
        .ok()?
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned)
}

struct Exec {
    ok: bool,
    text: String,
}

impl Exec {
    fn into_result(self, ok_summary: &str, fail_summary: &str) -> OpResult {
        let summary = if self.ok { ok_summary } else { fail_summary };
        OpResult {
            ok: self.ok,
            summary: if summary.is_empty() {
                first_line(&self.text)
            } else {
                summary.to_owned()
            },
            output: self.text,
        }
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("git exited with an error")
        .trim_start_matches("fatal: ")
        .trim_start_matches("error: ")
        .to_owned()
}

/// Runs a writing git command and keeps everything it printed. Unlike `git()` this
/// takes the normal locks (it changes the repository) and never fails on a
/// non-zero exit: git's message is the result.
fn exec(dir: &Path, args: &[&str], timeout: Duration) -> Result<Exec, GitError> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        // No editor can open here (pull of a merge would want one; ff-only never does).
        .env("GIT_EDITOR", "true")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => GitError::NotInstalled,
        _ => GitError::Failed(format!("could not start git: {err}")),
    })?;
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut bytes);
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let out = read(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let err = read(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GitError::Timeout(timeout));
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(GitError::Failed(format!("lost track of git: {e}"))),
        }
    };
    let text = [
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default(),
    ]
    .iter()
    .map(|s| s.trim())
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join("\n");
    Ok(Exec {
        ok: status.success(),
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_lines() {
        let b = parse_local("*\tmain\torigin/main\tahead 2, behind 1\t2 days ago").unwrap();
        assert!(b.current);
        assert_eq!(b.upstream.as_deref(), Some("origin/main"));
        assert_eq!((b.ahead, b.behind, b.gone), (2, 1, false));
        assert_eq!(b.updated, "2 days ago");
        let g = parse_local(" \told\torigin/old\tgone\t3 weeks ago").unwrap();
        assert!(!g.current && g.gone);
        let n = parse_local(" \tlocal-only\t\t\tnow").unwrap();
        assert_eq!(n.upstream, None);
        assert_eq!((n.ahead, n.behind), (0, 0));
    }

    #[test]
    fn ops_parse_from_json() {
        let op: GitOp = serde_json::from_str(r#"{"kind":"switch","branch":"dev"}"#).unwrap();
        assert_eq!(
            op,
            GitOp::Switch {
                branch: "dev".into()
            }
        );
        let op: GitOp = serde_json::from_str(r#"{"kind":"pull"}"#).unwrap();
        assert_eq!(op, GitOp::Pull);
    }
}
