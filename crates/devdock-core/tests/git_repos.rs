//! Integration tests against real repositories built in temp folders (T3.4).
//! Setup commands run with an empty global config so the developer's own git settings
//! (signing, hooks, default branch) can't change the outcome.

// Test setup should fail loudly; clippy's in-test exemption doesn't reach helper fns here.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use devdock_core::git::{self, GitError};
use devdock_core::paths;
use devdock_core::tools::which;

struct Sandbox {
    dir: tempfile::TempDir,
    empty_config: PathBuf,
}

impl Sandbox {
    /// `None` when git isn't installed, so the suite skips instead of failing.
    fn new() -> Option<Self> {
        which("git")?;
        let dir = tempfile::tempdir().unwrap();
        let empty_config = dir.path().join("empty.gitconfig");
        fs::write(&empty_config, "").unwrap();
        Some(Self { dir, empty_config })
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// Runs a setup git command and panics with stderr if it fails.
    fn git(&self, cwd: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_CONFIG_GLOBAL", &self.empty_config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
            .env("GIT_COMMITTER_NAME", "Test Author")
            .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(&self, rel: &str, text: &str) {
        let p = self.path(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    /// `remote.git` (bare) + `work` (clone with one commit on `main`, pushed).
    fn repo_with_upstream(&self) -> PathBuf {
        let remote = self.path("remote.git");
        let work = self.path("work");
        fs::create_dir_all(&remote).unwrap();
        fs::create_dir_all(&work).unwrap();
        self.git(&remote, &["init", "--bare", "-b", "main"]);
        self.git(&work, &["init", "-b", "main"]);
        self.write("work/README.md", "hello\n");
        self.write("work/apps/web/package.json", "{}\n");
        self.git(&work, &["add", "."]);
        self.git(&work, &["commit", "-m", "Initial commit"]);
        self.git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        self.git(&work, &["push", "-u", "origin", "main"]);
        work
    }
}

#[test]
fn dirty_repo_counts_match_and_upstream_is_tracked() {
    let Some(sb) = Sandbox::new() else { return };
    let work = sb.repo_with_upstream();

    sb.write("work/local.txt", "1\n");
    sb.git(&work, &["add", "local.txt"]);
    sb.git(&work, &["commit", "-m", "Local work"]); // ahead 1
    sb.write("work/README.md", "changed\n"); // modified
    sb.write("work/staged.txt", "s\n");
    sb.git(&work, &["add", "staged.txt"]); // staged
    sb.write("work/untracked.txt", "u\n"); // untracked

    let info = git::repo_info(&work).unwrap();
    let s = &info.status;
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!(s.upstream.as_deref(), Some("origin/main"));
    assert_eq!((s.ahead, s.behind), (Some(1), Some(0)));
    assert_eq!((s.staged, s.modified, s.untracked), (1, 1, 1));
    assert_eq!(s.changed_files, 3);

    assert_eq!(paths::key(&info.root), paths::key(&work));
    assert!(!info.in_subfolder && !info.is_worktree);
    let last = info.last_commit.unwrap();
    assert_eq!(last.subject, "Local work");
    assert_eq!(last.author, "Test Author");
    assert_eq!(info.remotes.len(), 1);
    assert_eq!(info.remotes[0].name, "origin");
    assert_eq!(info.worktrees.len(), 1);
    assert!(info.worktrees[0].is_current);
}

#[test]
fn behind_after_someone_else_pushes() {
    let Some(sb) = Sandbox::new() else { return };
    let work = sb.repo_with_upstream();
    let other = sb.path("other");
    sb.git(
        sb.dir.path(),
        &["clone", sb.path("remote.git").to_str().unwrap(), "other"],
    );
    sb.write("other/theirs.txt", "t\n");
    sb.git(&other, &["add", "."]);
    sb.git(&other, &["commit", "-m", "Their change"]);
    sb.git(&other, &["push"]);
    sb.git(&work, &["fetch"]);

    let s = git::status(&work).unwrap();
    assert_eq!((s.ahead, s.behind), (Some(0), Some(1)));
}

#[test]
fn monorepo_subfolder_reports_the_shared_root() {
    let Some(sb) = Sandbox::new() else { return };
    let work = sb.repo_with_upstream();
    sb.write("work/apps/web/page.tsx", "x\n"); // untracked, inside the subfolder

    let info = git::repo_info(&work.join("apps").join("web")).unwrap();
    assert!(info.in_subfolder);
    assert_eq!(paths::key(&info.root), paths::key(&work));
    // Counts are for the whole repository, exactly like `git status` run there.
    assert_eq!(info.status.untracked, 1);
}

#[test]
fn linked_worktree_is_detected() {
    let Some(sb) = Sandbox::new() else { return };
    let work = sb.repo_with_upstream();
    let wt = sb.path("work-feature");
    sb.git(
        &work,
        &["worktree", "add", wt.to_str().unwrap(), "-b", "feature/x"],
    );

    let info = git::repo_info(&wt).unwrap();
    assert!(info.is_worktree);
    assert!(!info.in_subfolder);
    assert_eq!(info.status.branch.as_deref(), Some("feature/x"));
    assert_eq!(info.status.upstream, None);
    assert_eq!(info.worktrees.len(), 2);
    let current: Vec<_> = info.worktrees.iter().filter(|w| w.is_current).collect();
    assert_eq!(current.len(), 1);
    assert_eq!(paths::key(&current[0].path), paths::key(&wt));

    // The main checkout is not a worktree.
    assert!(!git::repo_info(&work).unwrap().is_worktree);
}

#[test]
fn detached_head_has_no_branch() {
    let Some(sb) = Sandbox::new() else { return };
    let work = sb.repo_with_upstream();
    sb.git(&work, &["checkout", "--detach"]);
    let s = git::status(&work).unwrap();
    assert!(s.is_detached());
    assert_eq!(s.upstream, None);
}

#[test]
fn empty_repo_has_no_commit_yet() {
    let Some(sb) = Sandbox::new() else { return };
    let fresh = sb.path("fresh");
    fs::create_dir_all(&fresh).unwrap();
    sb.git(&fresh, &["init", "-b", "main"]);
    sb.write("fresh/a.txt", "a\n");

    let info = git::repo_info(&fresh).unwrap();
    assert_eq!(info.status.oid, None);
    assert_eq!(info.status.branch.as_deref(), Some("main"));
    assert_eq!(info.status.untracked, 1);
    assert!(info.last_commit.is_none());
    assert!(info.remotes.is_empty());
}

#[test]
fn staged_rename_counts_as_one_change() {
    let Some(sb) = Sandbox::new() else { return };
    let work = sb.repo_with_upstream();
    sb.git(&work, &["mv", "README.md", "READ ME.md"]);
    let s = git::status(&work).unwrap();
    assert_eq!((s.staged, s.modified, s.changed_files), (1, 0, 1));
}

#[test]
fn plain_folder_is_not_a_repo() {
    let Some(sb) = Sandbox::new() else { return };
    let plain = sb.path("plain");
    fs::create_dir_all(&plain).unwrap();
    assert!(matches!(git::repo_info(&plain), Err(GitError::NotARepo)));
    assert!(matches!(git::status(&plain), Err(GitError::NotARepo)));
}

#[test]
fn reading_never_modifies_the_repository() {
    let Some(sb) = Sandbox::new() else { return };
    let work = sb.repo_with_upstream();
    sb.write("work/README.md", "changed\n");
    let index = work.join(".git").join("index");
    let before = fs::read(&index).unwrap();
    for _ in 0..3 {
        git::repo_info(&work).unwrap();
    }
    assert_eq!(fs::read(&index).unwrap(), before, "the index was rewritten");
}

mod ops {
    use super::*;
    use devdock_core::git::ops::{self, GitOp};

    fn other_clone(sb: &Sandbox) -> PathBuf {
        let other = sb.path("other");
        sb.git(
            sb.dir.path(),
            &["clone", sb.path("remote.git").to_str().unwrap(), "other"],
        );
        other
    }

    fn current(dir: &Path) -> String {
        git::status(dir).unwrap().branch.unwrap()
    }

    #[test]
    fn create_switch_and_list_branches() {
        let Some(sb) = Sandbox::new() else { return };
        let work = sb.repo_with_upstream();

        let r = ops::run(
            &work,
            &GitOp::Create {
                name: "feature/login".into(),
            },
        )
        .unwrap();
        assert!(r.ok, "{r:?}");
        assert_eq!(current(&work), "feature/login");

        let b = ops::branches(&work).unwrap();
        let names: Vec<_> = b.local.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&"main") && names.contains(&"feature/login"));
        assert!(
            b.local
                .iter()
                .find(|l| l.name == "feature/login")
                .unwrap()
                .current
        );
        assert_eq!(
            b.local
                .iter()
                .find(|l| l.name == "main")
                .unwrap()
                .upstream
                .as_deref(),
            Some("origin/main")
        );

        let r = ops::run(
            &work,
            &GitOp::Switch {
                branch: "main".into(),
            },
        )
        .unwrap();
        assert!(r.ok && r.summary == "Switched to main", "{r:?}");
        assert_eq!(current(&work), "main");

        assert!(
            ops::run(
                &work,
                &GitOp::Create {
                    name: "bad name..".into()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn first_push_sets_the_upstream() {
        let Some(sb) = Sandbox::new() else { return };
        let work = sb.repo_with_upstream();
        assert!(
            ops::run(
                &work,
                &GitOp::Create {
                    name: "feat".into()
                }
            )
            .unwrap()
            .ok
        );
        sb.write("work/feat.txt", "f\n");
        sb.git(&work, &["add", "."]);
        sb.git(&work, &["commit", "-m", "Feature"]);

        let r = ops::run(&work, &GitOp::Push).unwrap();
        assert!(r.ok, "{r:?}");
        assert!(r.summary.contains("upstream"), "{r:?}");
        assert_eq!(
            git::status(&work).unwrap().upstream.as_deref(),
            Some("origin/feat")
        );
    }

    #[test]
    fn pull_fast_forwards_and_counts_commits() {
        let Some(sb) = Sandbox::new() else { return };
        let work = sb.repo_with_upstream();
        let other = other_clone(&sb);
        for i in 0..2 {
            sb.write(&format!("other/t{i}.txt"), "t\n");
            sb.git(&other, &["add", "."]);
            sb.git(&other, &["commit", "-m", "Theirs"]);
        }
        sb.git(&other, &["push"]);

        let r = ops::run(&work, &GitOp::Pull).unwrap();
        assert!(r.ok, "{r:?}");
        assert_eq!(r.summary, "Pulled 2 commits");
        let again = ops::run(&work, &GitOp::Pull).unwrap();
        assert_eq!(again.summary, "Already up to date");
    }

    #[test]
    fn diverged_pull_is_refused_with_a_plain_reason() {
        let Some(sb) = Sandbox::new() else { return };
        let work = sb.repo_with_upstream();
        let other = other_clone(&sb);
        sb.write("other/theirs.txt", "t\n");
        sb.git(&other, &["add", "."]);
        sb.git(&other, &["commit", "-m", "Theirs"]);
        sb.git(&other, &["push"]);
        sb.write("work/mine.txt", "m\n");
        sb.git(&work, &["add", "."]);
        sb.git(&work, &["commit", "-m", "Mine"]);

        let r = ops::run(&work, &GitOp::Pull).unwrap();
        assert!(!r.ok);
        assert!(r.summary.contains("both changed"), "{r:?}");
    }

    #[test]
    fn remote_only_branch_is_checked_out_with_tracking() {
        let Some(sb) = Sandbox::new() else { return };
        let work = sb.repo_with_upstream();
        let other = other_clone(&sb);
        sb.git(&other, &["switch", "-c", "review"]);
        sb.write("other/r.txt", "r\n");
        sb.git(&other, &["add", "."]);
        sb.git(&other, &["commit", "-m", "Review"]);
        sb.git(&other, &["push", "-u", "origin", "review"]);

        assert!(ops::run(&work, &GitOp::Fetch).unwrap().ok);
        let b = ops::branches(&work).unwrap();
        assert!(b.remote_only.contains(&"origin/review".to_owned()), "{b:?}");

        let r = ops::run(
            &work,
            &GitOp::Switch {
                branch: "origin/review".into(),
            },
        )
        .unwrap();
        assert!(r.ok, "{r:?}");
        assert_eq!(r.summary, "Switched to review");
        assert_eq!(current(&work), "review");
        assert_eq!(
            git::status(&work).unwrap().upstream.as_deref(),
            Some("origin/review")
        );
    }
}
