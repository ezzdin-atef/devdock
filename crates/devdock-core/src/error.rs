//! Typed errors plus the user-facing copy for each one (DESIGN.md section 8).
//! The UI only displays `ErrorDto`; it never writes its own error text.

use std::io;
use std::path::PathBuf;

use serde::Serialize;

use crate::agents::PlanError;
use crate::git::GitError;
use crate::ports::{ConfirmError, TerminateError};
use crate::project::ProjectId;
use crate::run::RunError;
use crate::store::StoreError;

/// Opening a folder in another program failed.
#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("folder not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("{0} wasn't found on PATH")]
    EditorMissing(String),
    #[error("could not start {program}: {source}")]
    Io {
        program: String,
        #[source]
        source: io::Error,
    },
}

/// Adding or looking up a project failed.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("folder not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("not a folder: {}", .0.display())]
    NotADirectory(PathBuf),
    #[error("already docked: {}", .0.display())]
    Duplicate(PathBuf),
    #[error("unknown project id {0:?}")]
    UnknownId(ProjectId),
    #[error("empty project name")]
    EmptyName,
    #[error("a scan is already running")]
    ScanRunning,
    #[error("scan root already added: {}", .0.display())]
    ScanRootExists(PathBuf),
    #[error("not a scan root: {}", .0.display())]
    ScanRootUnknown(PathBuf),
    #[error("{0}")]
    InvalidCommand(String),
    #[error("unknown command {0}")]
    UnknownCommand(String),
}

/// What crosses to the UI: what happened, and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDto {
    pub kind: &'static str,
    pub message: String,
    pub fix: Option<String>,
}

impl From<LaunchError> for ErrorDto {
    fn from(err: LaunchError) -> Self {
        match err {
            LaunchError::NotFound(path) => ErrorDto {
                kind: "notFound",
                message: format!("{} doesn't exist anymore.", path.display()),
                fix: Some(
                    "If it moved, use Locate… to point DevDock at it. Otherwise remove it from the list."
                        .into(),
                ),
            },
            LaunchError::EditorMissing(cmd) => ErrorDto {
                kind: "editorMissing",
                message: format!("{cmd} wasn't found on PATH, so DevDock can't open the editor."),
                fix: Some(
                    "Install the editor with its \"Add to PATH\" option, or pick another one in \
                     Settings. DevDock checks again on the next click. Explorer still works."
                        .into(),
                ),
            },
            LaunchError::Io { program, source } => ErrorDto {
                kind: "io",
                message: format!("Windows couldn't start {program}: {source}."),
                fix: Some(
                    "Try again. If it keeps failing, check that the program still exists.".into(),
                ),
            },
        }
    }
}

impl From<ProjectError> for ErrorDto {
    fn from(err: ProjectError) -> Self {
        match err {
            ProjectError::NotFound(path) => ErrorDto {
                kind: "notFound",
                message: format!("{} doesn't exist.", path.display()),
                fix: Some("Pick a folder that exists.".into()),
            },
            ProjectError::NotADirectory(path) => ErrorDto {
                kind: "notADirectory",
                message: format!("{} is a file, not a folder.", path.display()),
                fix: Some("Pick the project folder instead.".into()),
            },
            ProjectError::Duplicate(path) => ErrorDto {
                kind: "duplicate",
                message: format!("{} is already docked.", path.display()),
                fix: None,
            },
            ProjectError::UnknownId(_) => ErrorDto {
                kind: "unknownProject",
                message: "That project is no longer in the list.".into(),
                fix: Some("Pick it again from the list.".into()),
            },
            ProjectError::EmptyName => ErrorDto {
                kind: "emptyName",
                message: "A project needs a name.".into(),
                fix: Some("Type a name, or press Esc to keep the old one.".into()),
            },
            ProjectError::ScanRunning => ErrorDto {
                kind: "scanRunning",
                message: "A scan is already running.".into(),
                fix: Some("Wait for it to finish, then try again.".into()),
            },
            ProjectError::ScanRootExists(path) => ErrorDto {
                kind: "scanRootExists",
                message: format!("{} is already a scan root.", path.display()),
                fix: Some("Use Rescan to look for new projects in it.".into()),
            },
            ProjectError::InvalidCommand(msg) => ErrorDto {
                kind: "invalidCommand",
                message: msg,
                fix: None,
            },
            ProjectError::UnknownCommand(_) => ErrorDto {
                kind: "unknownCommand",
                message: "That command is no longer saved for this project.".into(),
                fix: None,
            },
            ProjectError::ScanRootUnknown(path) => ErrorDto {
                kind: "scanRootUnknown",
                message: format!("{} is not a scan root.", path.display()),
                fix: None,
            },
        }
    }
}

impl From<StoreError> for ErrorDto {
    fn from(err: StoreError) -> Self {
        ErrorDto {
            kind: "storeError",
            message: format!("DevDock {err}."),
            fix: Some(
                "Your changes are kept in memory and DevDock retries on the next change.                  Check that the folder isn't read-only or locked by another program."
                    .into(),
            ),
        }
    }
}

impl From<GitError> for ErrorDto {
    fn from(err: GitError) -> Self {
        let (kind, message, fix): (&'static str, String, Option<&str>) = match &err {
            GitError::NotInstalled => (
                "gitMissing",
                "git wasn't found on PATH, so DevDock can't read repository state.".into(),
                Some(
                    "Install Git for Windows (keep \"Git from the command line\" selected), then press Refresh.",
                ),
            ),
            GitError::NotARepo => (
                "notARepo",
                "This folder isn't inside a Git repository.".into(),
                None,
            ),
            GitError::DubiousOwnership => (
                "gitDubiousOwnership",
                "Git won't read this repository because it belongs to another Windows user.".into(),
                Some("If you trust it, run: git config --global --add safe.directory <repo path>"),
            ),
            GitError::Timeout(_) => (
                "gitTimeout",
                format!("{err}, so DevDock stopped waiting."),
                Some(
                    "Large repositories and network drives can be slow. Press Refresh to try again.",
                ),
            ),
            GitError::Failed(detail) => (
                "gitFailed",
                format!("git reported: {detail}."),
                Some("Run git status in that folder to see the full message."),
            ),
        };
        ErrorDto {
            kind,
            message,
            fix: fix.map(str::to_owned),
        }
    }
}

impl From<RunError> for ErrorDto {
    fn from(err: RunError) -> Self {
        let (kind, fix): (&'static str, Option<&str>) = match &err {
            RunError::AlreadyRunning(_) => {
                ("alreadyRunning", Some("Stop it first, or use Restart."))
            }
            RunError::NotRunning(_) => ("notRunning", None),
            RunError::Spawn { .. } => (
                "spawnFailed",
                Some(
                    "Check that the program is installed and on PATH, and that the folder exists.",
                ),
            ),
            RunError::Kill(_) => (
                "killFailed",
                Some("The processes may already have ended. If not, check Task Manager."),
            ),
            RunError::StopTimeout(..) => ("stopTimeout", Some("Try Stop again in a moment.")),
            RunError::Invalid(_) => ("unexpected", None),
        };
        let mut message = err.to_string();
        if let Some(first) = message.get(..1) {
            message = first.to_uppercase() + &message[1..];
        }
        ErrorDto {
            kind,
            message: format!("{message}."),
            fix: fix.map(str::to_owned),
        }
    }
}

impl From<ConfirmError> for ErrorDto {
    fn from(err: ConfirmError) -> Self {
        let (kind, fix) = match &err {
            ConfirmError::Gone(_) => (
                "processGone",
                Some("Nothing to do; refresh the Ports list."),
            ),
            ConfirmError::Changed(_) => (
                "processChanged",
                Some("DevDock ended nothing. Refresh the Ports list and check the process again."),
            ),
            ConfirmError::Protected { .. } => (
                "processProtected",
                Some(
                    "DevDock never ends system processes. Stop the service from Windows if you must.",
                ),
            ),
            ConfirmError::Owned { .. } => ("processOwned", Some("Use Stop on its row.")),
        };
        ErrorDto {
            kind,
            message: format!("{err}."),
            fix: fix.map(str::to_owned),
        }
    }
}

impl From<TerminateError> for ErrorDto {
    fn from(err: TerminateError) -> Self {
        match err {
            TerminateError::AccessDenied { pid, name } => ErrorDto {
                kind: "accessDenied",
                message: format!(
                    "Windows denied access to {name} (PID {pid}). It belongs to another user or runs elevated."
                ),
                fix: Some("Run DevDock as administrator, or end it from Task Manager.".into()),
            },
            TerminateError::Gone(pid) => ErrorDto {
                kind: "processGone",
                message: format!("PID {pid} had already exited."),
                fix: None,
            },
            TerminateError::Other(e) => ErrorDto {
                kind: "terminateFailed",
                message: format!("Windows couldn't end the process: {e}."),
                fix: Some("Try again, or end it from Task Manager.".into()),
            },
        }
    }
}

impl From<PlanError> for ErrorDto {
    fn from(err: PlanError) -> Self {
        match err {
            PlanError::TerminalMissing(t) => ErrorDto {
                kind: "terminalMissing",
                message: format!("{} wasn't found, so DevDock can't open it.", t.name()),
                fix: Some(
                    "Install it, or choose another terminal. Settings lists what DevDock found."
                        .into(),
                ),
            },
            PlanError::UnsafeProgram(p) => ErrorDto {
                kind: "unsafeProgram",
                message: format!("DevDock won't pass {p} to a shell."),
                fix: None,
            },
        }
    }
}

impl ErrorDto {
    /// A startup notice about loading saved data (not an error the user caused).
    pub fn store_warning(message: String) -> Self {
        ErrorDto {
            kind: "storeWarning",
            message,
            fix: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_missing_names_the_command_and_the_fix() {
        let dto = ErrorDto::from(LaunchError::EditorMissing("code".into()));
        assert_eq!(dto.kind, "editorMissing");
        assert!(dto.message.starts_with("code wasn't found on PATH"));
        assert!(dto.fix.unwrap().contains("Explorer still works"));
    }

    #[test]
    fn not_found_uses_the_real_path() {
        let dto = ErrorDto::from(LaunchError::NotFound("D:\\dev\\gone".into()));
        assert!(dto.message.contains("D:\\dev\\gone"));
    }

    #[test]
    fn duplicate_has_no_fix() {
        let dto = ErrorDto::from(ProjectError::Duplicate("D:\\dev\\x".into()));
        assert_eq!(dto.fix, None);
    }

    #[test]
    fn dto_serializes_camel_case() {
        let dto = ErrorDto::from(ProjectError::UnknownId(ProjectId("ab".into())));
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["kind"], "unknownProject");
        assert!(json.get("fix").is_some());
    }
}
