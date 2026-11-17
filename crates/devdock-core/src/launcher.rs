//! Opening a project folder in Explorer or the editor.
//! Launches are fire-and-forget: DevDock does not own or wait for these processes.

use std::path::Path;
use std::process::Command;

use crate::error::LaunchError;

/// Windows `CREATE_NO_WINDOW`: stops a console flashing up when running `code.cmd`.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn open_in_explorer(folder: &Path, explorer: Option<&Path>) -> Result<(), LaunchError> {
    ensure_exists(folder)?;
    let program = explorer.unwrap_or(Path::new("explorer.exe"));
    let mut cmd = Command::new(program);
    cmd.arg(folder);
    spawn(cmd, "explorer.exe")
}

/// `editor` is the resolved path of the editor command (see `tools::which`),
/// or `None` when it wasn't found on PATH.
pub fn open_in_editor(
    folder: &Path,
    editor_cmd: &str,
    editor: Option<&Path>,
) -> Result<(), LaunchError> {
    ensure_exists(folder)?;
    let program = editor.ok_or_else(|| LaunchError::EditorMissing(editor_cmd.to_owned()))?;
    let mut cmd = Command::new(program);
    cmd.arg(folder);
    spawn(cmd, editor_cmd)
}

/// Opens a terminal (optionally running an agent) as described by `agents::plan`.
/// The terminal is not part of any DevDock job: it outlives DevDock, like any
/// window the user opened themselves.
pub fn spawn_plan(plan: &crate::agents::LaunchPlan, label: &str) -> Result<(), LaunchError> {
    spawn_plan_child(plan, label).map(drop)
}

/// Like [`spawn_plan`] but hands back the child (tests use it to clean up).
pub fn spawn_plan_child(
    plan: &crate::agents::LaunchPlan,
    label: &str,
) -> Result<std::process::Child, LaunchError> {
    use crate::agents::Arg;
    ensure_exists(&plan.cwd)?;
    let mut cmd = Command::new(&plan.program);
    cmd.current_dir(&plan.cwd)
        .envs(plan.env.iter().map(|(k, v)| (k, v)));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        for arg in &plan.args {
            match arg {
                Arg::Plain(a) => {
                    cmd.arg(a);
                }
                Arg::Raw(a) => {
                    cmd.raw_arg(a);
                }
            }
        }
        // CREATE_NEW_CONSOLE gives console shells their own window.
        if plan.new_console {
            cmd.creation_flags(0x0000_0010);
        }
    }
    #[cfg(not(windows))]
    for arg in &plan.args {
        match arg {
            Arg::Plain(a) | Arg::Raw(a) => {
                cmd.arg(a);
            }
        }
    }
    cmd.spawn().map_err(|source| LaunchError::Io {
        program: label.to_owned(),
        source,
    })
}

fn ensure_exists(folder: &Path) -> Result<(), LaunchError> {
    if folder.is_dir() {
        Ok(())
    } else {
        Err(LaunchError::NotFound(folder.to_path_buf()))
    }
}

fn spawn(mut cmd: Command, program: &str) -> Result<(), LaunchError> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.spawn().map(drop).map_err(|source| LaunchError::Io {
        program: program.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_folder_is_not_found_before_anything_runs() {
        let err = open_in_explorer(Path::new("Z:\\definitely\\not\\here"), None).unwrap_err();
        assert!(matches!(err, LaunchError::NotFound(_)));
    }

    #[test]
    fn missing_editor_is_reported_with_its_command_name() {
        let dir = tempfile::tempdir().unwrap();
        let err = open_in_editor(dir.path(), "code", None).unwrap_err();
        assert!(matches!(err, LaunchError::EditorMissing(ref c) if c == "code"));
    }

    #[test]
    fn missing_folder_wins_over_missing_editor() {
        let err = open_in_editor(Path::new("Z:\\nope"), "code", None).unwrap_err();
        assert!(matches!(err, LaunchError::NotFound(_)));
    }

    #[test]
    fn unstartable_program_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let bogus = dir.path().join("no-such-editor.exe");
        let err = open_in_editor(dir.path(), "code", Some(&bogus)).unwrap_err();
        assert!(matches!(err, LaunchError::Io { .. }));
    }
}
