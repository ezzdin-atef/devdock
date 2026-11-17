//! Running the git CLI with a timeout (T3.1). Read-only by construction: every call
//! passes `--no-optional-locks` so even `git status` never rewrites the index.

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::GitError;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Runs `git <args>` in `dir` and returns stdout. Kills git if it runs past `timeout`.
pub fn git(dir: &Path, args: &[&str], timeout: Duration) -> Result<String, GitError> {
    let mut cmd = Command::new("git");
    cmd.arg("--no-optional-locks")
        .args(args)
        .current_dir(dir)
        // Never wait for a password or editor; DevDock has no terminal to show it in.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => GitError::NotInstalled,
        _ => GitError::Failed(format!("could not start git: {err}")),
    })?;

    // Drain both pipes on their own threads so a chatty git can't fill a pipe
    // buffer and block forever while we wait for it to exit.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let status = wait_with_timeout(&mut child, timeout)?;
    let out = stdout.join().unwrap_or_default();
    let err = stderr.join().unwrap_or_default();

    if status.success() {
        Ok(out)
    } else {
        Err(classify_failure(&err))
    }
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut p) = pipe {
            let mut bytes = Vec::new();
            let _ = p.read_to_end(&mut bytes);
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
        text
    })
}

fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, GitError> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GitError::Timeout(timeout));
            }
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(err) => return Err(GitError::Failed(format!("lost track of git: {err}"))),
        }
    }
}

/// Turns git's stderr into a typed error. Matches the stable English phrases git
/// prints with `LC_ALL` unset; anything else keeps the first line of stderr.
pub fn classify_failure(stderr: &str) -> GitError {
    let lower = stderr.to_lowercase();
    if lower.contains("not a git repository") {
        GitError::NotARepo
    } else if lower.contains("detected dubious ownership") {
        GitError::DubiousOwnership
    } else {
        let first = stderr
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("git exited with an error");
        GitError::Failed(first.trim_start_matches("fatal: ").to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_a_repo_is_recognised() {
        let e = classify_failure(
            "fatal: not a git repository (or any of the parent directories): .git\n",
        );
        assert!(matches!(e, GitError::NotARepo));
    }

    #[test]
    fn dubious_ownership_is_recognised() {
        let e = classify_failure("fatal: detected dubious ownership in repository at 'D:/x'\n");
        assert!(matches!(e, GitError::DubiousOwnership));
    }

    #[test]
    fn other_failures_keep_the_first_line_without_fatal() {
        let e = classify_failure("\nfatal: bad revision 'HEAD'\nmore\n");
        assert!(matches!(e, GitError::Failed(ref m) if m == "bad revision 'HEAD'"));
    }

    #[test]
    fn plain_folder_is_not_a_repo() {
        // A temp folder is not inside a repo (the system temp dir isn't one).
        let dir = tempfile::tempdir().unwrap();
        match git(
            dir.path(),
            &["rev-parse", "--show-toplevel"],
            DEFAULT_TIMEOUT,
        ) {
            Err(GitError::NotARepo) | Err(GitError::NotInstalled) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}
