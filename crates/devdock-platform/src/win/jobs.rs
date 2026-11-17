//! Process trees on Windows with Job Objects (T4.2b).
//!
//! Every run gets its own job with `KILL_ON_JOB_CLOSE`. The shell is created
//! *suspended*, put in the job, then resumed, so not even its first child can start
//! outside the job. Stopping terminates the job: the shell, `npm`, `node` and anything
//! they started end together. If DevDock itself exits or crashes, Windows closes the
//! job handle and the tree dies with it, so no orphan keeps a port busy.
//!
//! `unsafe` in DevDock lives only in this crate's `win` modules. Each block is a
//! single Win32 call with its invariants written next to it.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};

use devdock_core::run::{ProcessTree, SpawnSpec, Spawned, TreeHandle};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicAccountingInformation, JobObjectBasicProcessIdList,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};
use windows::Win32::System::Threading::{
    CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread,
    THREAD_SUSPEND_RESUME,
};
use windows::core::PCWSTR;

/// Exit code given to processes ended by Stop.
const STOPPED_EXIT_CODE: u32 = 1;

fn win_err(err: windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error(err.code().0)
}

/// A kernel handle closed on drop.
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from a successful Win32 call that returned a new
        // handle owned by this struct, and it is closed exactly once, here.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// A job object with kill-on-close. Closing the last handle ends every process in it.
struct Job(OwnedHandle);

// SAFETY: a job handle is a process-wide kernel handle; the Job Object functions used
// here are thread-safe, and the handle value is only read after creation.
unsafe impl Send for Job {}
// SAFETY: as above; `&Job` only allows calls that the kernel synchronises itself.
unsafe impl Sync for Job {}

impl Job {
    fn new() -> io::Result<Self> {
        // SAFETY: no security attributes and no name are valid arguments; on success
        // we receive a new handle that `OwnedHandle` takes ownership of.
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(win_err)?;
        let job = Job(OwnedHandle(handle));

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` is a properly initialised JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        // the pointer and size describe exactly that struct, and it outlives the call.
        unsafe {
            SetInformationJobObject(
                job.handle(),
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast::<c_void>(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(win_err)?;
        Ok(job)
    }

    fn handle(&self) -> HANDLE {
        (self.0).0
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        let process = HANDLE(child.as_raw_handle());
        // SAFETY: both handles are valid for the duration of the call: the job is
        // owned by `self`, and `child` keeps its process handle open until dropped.
        unsafe { AssignProcessToJobObject(self.handle(), process) }.map_err(win_err)
    }

    fn terminate(&self) -> io::Result<()> {
        // SAFETY: the job handle is valid; terminating an already-empty job is a no-op.
        unsafe { TerminateJobObject(self.handle(), STOPPED_EXIT_CODE) }.map_err(win_err)
    }

    fn active_processes(&self) -> io::Result<usize> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: the out-pointer and size describe `info` exactly, `info` lives across
        // the call, and the return-length out-parameter is optional.
        unsafe {
            QueryInformationJobObject(
                Some(self.handle()),
                JobObjectBasicAccountingInformation,
                (&raw mut info).cast::<c_void>(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                None,
            )
        }
        .map_err(win_err)?;
        Ok(info.ActiveProcesses as usize)
    }

    /// PIDs currently in the job (up to `MAX_JOB_PIDS`).
    fn pids(&self) -> io::Result<Vec<u32>> {
        let mut list = PidList {
            assigned: 0,
            listed: 0,
            ids: [0; MAX_JOB_PIDS],
        };
        // SAFETY: `PidList` has the layout of JOBOBJECT_BASIC_PROCESS_ID_LIST with a
        // larger trailing array (two u32 counters, then usize ids); the size passed is
        // exactly `size_of::<PidList>()` and `list` outlives the call.
        unsafe {
            QueryInformationJobObject(
                Some(self.handle()),
                JobObjectBasicProcessIdList,
                (&raw mut list).cast::<c_void>(),
                size_of::<PidList>() as u32,
                None,
            )
        }
        .map_err(win_err)?;
        let n = (list.listed as usize).min(MAX_JOB_PIDS);
        Ok(list.ids[..n].iter().map(|&id| id as u32).collect())
    }
}

/// Enough for any dev server tree; Windows reports more data if a job is bigger,
/// and the call then fails rather than truncating, which `pids()` reports as empty.
const MAX_JOB_PIDS: usize = 1024;

#[repr(C)]
struct PidList {
    assigned: u32,
    listed: u32,
    ids: [usize; MAX_JOB_PIDS],
}

/// Resumes the (single) main thread of a process created with `CREATE_SUSPENDED`.
/// `std` doesn't expose the thread handle, so it's found with a thread snapshot.
fn resume_process(pid: u32) -> io::Result<()> {
    // SAFETY: TH32CS_SNAPTHREAD with pid 0 snapshots all threads; the returned handle
    // is owned by `OwnedHandle`.
    let snapshot =
        OwnedHandle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.map_err(win_err)?);
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut resumed = 0;
    // SAFETY: `entry.dwSize` is set as the API requires and `entry` outlives the call.
    let mut more = unsafe { Thread32First(snapshot.0, &raw mut entry) }.is_ok();
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: opening a thread by id has no memory-safety preconditions; the
            // returned handle is owned by `OwnedHandle`.
            let thread = OwnedHandle(
                unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) }
                    .map_err(win_err)?,
            );
            // SAFETY: `thread` is a valid handle with THREAD_SUSPEND_RESUME access.
            if unsafe { ResumeThread(thread.0) } != u32::MAX {
                resumed += 1;
            }
        }
        // SAFETY: same snapshot and entry as above.
        more = unsafe { Thread32Next(snapshot.0, &raw mut entry) }.is_ok();
    }
    if resumed == 0 {
        return Err(io::Error::other(format!("could not resume process {pid}")));
    }
    Ok(())
}

fn shell() -> std::ffi::OsString {
    std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into())
}

/// Runs commands through `cmd /D /S /C "<line>"` inside a per-run Job Object.
#[derive(Debug, Default, Clone, Copy)]
pub struct JobTree;

pub struct JobHandle {
    job: Job,
}

impl TreeHandle for JobHandle {
    fn kill_tree(&self) -> io::Result<()> {
        self.job.terminate()
    }

    fn process_count(&self) -> Option<usize> {
        self.job.active_processes().ok()
    }

    fn pids(&self) -> Vec<u32> {
        self.job.pids().unwrap_or_default()
    }
}

impl ProcessTree for JobTree {
    fn spawn(&self, spec: &SpawnSpec) -> io::Result<Spawned> {
        let job = Job::new()?;
        let mut cmd = Command::new(shell());
        // `/S` + outer quotes: cmd strips exactly the first and last quote and runs the
        // rest verbatim, so the user's own quoting survives. `/D` skips AutoRun hooks.
        cmd.raw_arg(format!("/D /S /C \"{}\"", spec.command_line))
            .current_dir(&spec.cwd)
            .envs(spec.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags((CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP).0);
        let mut child = cmd.spawn()?;
        let pid = child.id();

        let started = job.assign(&child).and_then(|()| resume_process(pid));
        if let Err(err) = started {
            // Never leave a suspended shell behind.
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }
        Ok(Spawned {
            pid,
            child,
            tree: Box::new(JobHandle { job }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn spec(line: &str) -> SpawnSpec {
        SpawnSpec {
            command_line: line.into(),
            cwd: std::env::temp_dir(),
            env: vec![("DEVDOCK_TEST".into(), "1".into())],
        }
    }

    fn wait_for(mut cond: impl FnMut() -> bool, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if cond() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        cond()
    }

    #[test]
    fn output_and_exit_code_come_through() {
        let s = JobTree.spawn(&spec("echo %DEVDOCK_TEST%& exit 4")).unwrap();
        let out = s.child.wait_with_output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
        assert_eq!(out.status.code(), Some(4));
    }

    #[test]
    fn kill_tree_ends_grandchildren_too() {
        // cmd starts a background ping (grandchild) and a foreground ping.
        let mut s = JobTree
            .spawn(&spec(
                "start /B ping -n 60 127.0.0.1 >nul & ping -n 60 127.0.0.1 >nul",
            ))
            .unwrap();
        assert!(
            wait_for(
                || s.tree.process_count().unwrap_or(0) >= 3,
                Duration::from_secs(5)
            ),
            "expected cmd + 2 pings, got {:?}",
            s.tree.process_count()
        );

        let pids = s.tree.pids();
        assert!(
            pids.contains(&s.pid) && pids.len() >= 3,
            "job pids: {pids:?}"
        );

        s.tree.kill_tree().unwrap();
        assert!(
            wait_for(|| s.tree.process_count() == Some(0), Duration::from_secs(5)),
            "processes left in the job: {:?}",
            s.tree.process_count()
        );
        assert_eq!(
            s.child.wait().unwrap().code(),
            Some(STOPPED_EXIT_CODE as i32)
        );
        s.tree.kill_tree().unwrap(); // second kill is harmless
    }

    #[test]
    fn dropping_the_handle_kills_the_tree() {
        let mut s = JobTree.spawn(&spec("ping -n 60 127.0.0.1 >nul")).unwrap();
        drop(s.tree);
        assert!(wait_for(
            || matches!(s.child.try_wait(), Ok(Some(_))),
            Duration::from_secs(5)
        ));
    }

    #[test]
    fn bad_folder_is_an_error_not_a_hang() {
        let mut bad = spec("echo hi");
        bad.cwd = "Z:\\no\\such\\folder".into();
        assert!(JobTree.spawn(&bad).is_err());
    }

    #[test]
    fn quotes_in_the_command_line_survive() {
        let s = JobTree.spawn(&spec(r#"echo "a b"& echo c"#)).unwrap();
        let out = s.child.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("\"a b\""), "{text}");
    }
}
