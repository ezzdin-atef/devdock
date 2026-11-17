//! A short, one-off shell command with a timeout and no window (e.g.
//! `docker compose stop db`). Not supervised and not logged: use `Runner` for that.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Runs `line` through the shell in `cwd`. Returns the exit code, or `None` if it was
/// killed for running past `timeout`.
pub fn quick_shell(line: &str, cwd: &Path, timeout: Duration) -> io::Result<Option<i32>> {
    let mut cmd = shell(line);
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = cmd.spawn()?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.code());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(windows)]
fn shell(line: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new("cmd");
    c.raw_arg(format!("/D /S /C \"{line}\""))
        .creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    c
}

#[cfg(not(windows))]
fn shell(line: &str) -> Command {
    let mut c = Command::new("sh");
    c.args(["-c", line]);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_the_exit_code() {
        let dir = std::env::temp_dir();
        assert_eq!(
            quick_shell("exit 3", &dir, Duration::from_secs(10)).unwrap(),
            Some(3)
        );
    }

    #[test]
    fn kills_after_the_timeout() {
        let dir = std::env::temp_dir();
        let start = Instant::now();
        let got = quick_shell(
            "ping -n 30 127.0.0.1 >nul",
            &dir,
            Duration::from_millis(400),
        )
        .unwrap();
        assert_eq!(got, None);
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
