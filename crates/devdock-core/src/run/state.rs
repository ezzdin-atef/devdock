//! The run state machine (T4.4). Every change goes through `next`, so an
//! impossible transition (e.g. "stopped" before "started") is an error, not a bug.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RunState {
    #[default]
    Idle,
    Starting,
    Running {
        pid: u32,
    },
    /// Stop was requested; waiting for the process tree to end.
    Stopping {
        pid: u32,
    },
    /// Ended because the user stopped it.
    Stopped,
    /// Ended on its own with exit code 0.
    Exited {
        code: i32,
    },
    /// Ended on its own with a non-zero code, or never started (`code: None`).
    Failed {
        code: Option<i32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Start,
    Spawned {
        pid: u32,
    },
    SpawnFailed,
    StopRequested,
    /// `None` when Windows gives no code (the process was terminated).
    ProcessExited {
        code: Option<i32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("can't go from {from:?} with {transition:?}")]
pub struct InvalidTransition {
    pub from: RunState,
    pub transition: Transition,
}

impl RunState {
    pub fn next(&self, t: Transition) -> Result<RunState, InvalidTransition> {
        use RunState as S;
        use Transition as T;
        let to = match (self, t) {
            (S::Idle | S::Stopped | S::Exited { .. } | S::Failed { .. }, T::Start) => S::Starting,
            (S::Starting, T::Spawned { pid }) => S::Running { pid },
            (S::Starting, T::SpawnFailed) => S::Failed { code: None },
            (S::Running { pid } | S::Stopping { pid }, T::StopRequested) => {
                S::Stopping { pid: *pid }
            }
            (S::Running { .. }, T::ProcessExited { code: Some(0) }) => S::Exited { code: 0 },
            (S::Running { .. }, T::ProcessExited { code }) => S::Failed { code },
            (S::Stopping { .. }, T::ProcessExited { .. }) => S::Stopped,
            _ => {
                return Err(InvalidTransition {
                    from: self.clone(),
                    transition: t,
                });
            }
        };
        Ok(to)
    }

    /// A process exists (or is being created) for this run.
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            RunState::Starting | RunState::Running { .. } | RunState::Stopping { .. }
        )
    }

    pub fn pid(&self) -> Option<u32> {
        match self {
            RunState::Running { pid } | RunState::Stopping { pid } => Some(*pid),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use RunState as S;
    use Transition as T;

    fn walk(steps: &[Transition]) -> Result<RunState, InvalidTransition> {
        steps.iter().try_fold(S::Idle, |s, t| s.next(*t))
    }

    #[test]
    fn happy_path_exit_zero() {
        let s = walk(&[
            T::Start,
            T::Spawned { pid: 7 },
            T::ProcessExited { code: Some(0) },
        ]);
        assert_eq!(s, Ok(S::Exited { code: 0 }));
    }

    #[test]
    fn non_zero_exit_is_failed() {
        let s = walk(&[
            T::Start,
            T::Spawned { pid: 7 },
            T::ProcessExited { code: Some(3) },
        ]);
        assert_eq!(s, Ok(S::Failed { code: Some(3) }));
    }

    #[test]
    fn user_stop_is_stopped_whatever_the_code() {
        let s = walk(&[
            T::Start,
            T::Spawned { pid: 7 },
            T::StopRequested,
            T::ProcessExited { code: Some(1) },
        ]);
        assert_eq!(s, Ok(S::Stopped));
    }

    #[test]
    fn spawn_failure() {
        assert_eq!(
            walk(&[T::Start, T::SpawnFailed]),
            Ok(S::Failed { code: None })
        );
    }

    #[test]
    fn stop_twice_is_harmless() {
        let s = walk(&[
            T::Start,
            T::Spawned { pid: 7 },
            T::StopRequested,
            T::StopRequested,
        ]);
        assert_eq!(s, Ok(S::Stopping { pid: 7 }));
    }

    #[test]
    fn restart_from_every_finished_state() {
        for done in [
            S::Stopped,
            S::Exited { code: 0 },
            S::Failed { code: Some(1) },
        ] {
            assert_eq!(done.next(T::Start), Ok(S::Starting));
        }
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        assert!(S::Idle.next(T::StopRequested).is_err());
        assert!(S::Running { pid: 1 }.next(T::Start).is_err());
        assert!(S::Idle.next(T::ProcessExited { code: Some(0) }).is_err());
        assert!(S::Stopped.next(T::Spawned { pid: 1 }).is_err());
    }

    #[test]
    fn active_and_pid() {
        assert!(S::Running { pid: 4 }.is_active());
        assert!(!S::Failed { code: None }.is_active());
        assert_eq!(S::Stopping { pid: 4 }.pid(), Some(4));
    }

    #[test]
    fn serializes_tagged() {
        let v = serde_json::to_value(S::Failed { code: Some(2) }).unwrap();
        assert_eq!(v["state"], "failed");
        assert_eq!(v["code"], 2);
    }
}
