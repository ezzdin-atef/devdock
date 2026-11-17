//! The real [`ServiceDriver`]: services run through the same `Runner` as the Run tab,
//! so they show up there, on the Running screen and as DEVDOCK on Ports.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::health::{CHECK_TIMEOUT, check};
use super::model::{Service, ServiceSource};
use super::orchestrator::ServiceDriver;
use crate::paths;
use crate::project::ProjectId;
use crate::run::{CommandId, RunError, RunKey, RunLabel, Runner, SpawnSpec};
use crate::state::{AppState, lock};

pub struct RunnerDriver {
    pub runner: Arc<Runner>,
    pub state: Arc<Mutex<AppState>>,
    /// Shown in the Running screen's project column ("Wallet · api").
    pub workspace_name: String,
}

/// The run key a service uses. Commands share the Run tab's key, so starting a
/// workspace adopts a `dev` that is already running instead of starting a second one.
pub fn run_key(svc: &Service) -> RunKey {
    match &svc.source {
        ServiceSource::Command {
            project_id,
            command_id,
            worktree,
        } => RunKey {
            project_id: project_id.clone(),
            command_id: match worktree {
                None => command_id.clone(),
                // A worktree run is a different process from the main checkout's.
                Some(w) => CommandId(format!(
                    "{}@{:08x}",
                    command_id.0,
                    paths::stable_hash(&paths::key(w)) as u32
                )),
            },
        },
        ServiceSource::Compose {
            project_id,
            file,
            service,
        } => RunKey {
            project_id: project_id.clone(),
            command_id: CommandId(format!("compose:{file}:{service}")),
        },
    }
}

/// `docker compose -f <file> up <service>` (foreground, so logs stream in).
pub fn compose_up_line(file: &str, service: &str) -> String {
    format!("docker compose -f \"{file}\" up {service}")
}

pub fn compose_stop_line(file: &str, service: &str) -> String {
    format!("docker compose -f \"{file}\" stop {service}")
}

/// Rewrites a command's folder from the main checkout into a worktree:
/// `<project>/apps/web` -> `<worktree>/apps/web`.
pub fn rebase_into_worktree(cwd: &Path, project_root: &Path, worktree: &Path) -> PathBuf {
    cwd.strip_prefix(project_root)
        .map_or_else(|_| worktree.to_path_buf(), |rel| worktree.join(rel))
}

impl RunnerDriver {
    fn spec(&self, svc: &Service) -> Result<(RunLabel, SpawnSpec), String> {
        let state = lock(&self.state);
        let project_id: &ProjectId = svc.source.project_id();
        let project = state.get(project_id).map_err(|e| e.to_string())?;
        let label = RunLabel {
            project_name: format!("{} · {}", self.workspace_name, project.name),
            command_name: svc.name.clone(),
        };
        let spec = match &svc.source {
            ServiceSource::Command {
                command_id,
                worktree,
                ..
            } => {
                let cmd = state
                    .commands(project_id)
                    .map_err(|e| e.to_string())?
                    .commands
                    .into_iter()
                    .find(|c| &c.id == command_id)
                    .ok_or_else(|| format!("the command {} no longer exists", command_id.0))?;
                let cwd = match worktree {
                    Some(w) => rebase_into_worktree(&cmd.cwd, &project.path, w),
                    None => cmd.cwd,
                };
                SpawnSpec {
                    command_line: cmd.command_line,
                    cwd,
                    env: Vec::new(),
                }
            }
            ServiceSource::Compose { file, service, .. } => SpawnSpec {
                command_line: compose_up_line(file, service),
                cwd: project.path.clone(),
                env: Vec::new(),
            },
        };
        Ok((label, spec))
    }
}

impl ServiceDriver for RunnerDriver {
    fn start(&self, svc: &Service) -> Result<(), String> {
        let (label, spec) = self.spec(svc)?;
        match self.runner.start(run_key(svc), label, spec) {
            Ok(()) | Err(RunError::AlreadyRunning(_)) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn stop(&self, svc: &Service) {
        if let ServiceSource::Compose {
            project_id,
            file,
            service,
        } = &svc.source
        {
            // Containers outlive the `up` client: ask Compose to stop the service too.
            let cwd = lock(&self.state).get(project_id).map(|p| p.path.clone());
            if let Ok(cwd) = cwd {
                let _ = crate::run::quick_shell(
                    &compose_stop_line(file, service),
                    &cwd,
                    Duration::from_secs(30),
                );
            }
        }
        let _ = self.runner.stop(&run_key(svc));
    }

    fn is_running(&self, svc: &Service) -> bool {
        self.runner
            .info(&run_key(svc))
            .is_some_and(|r| r.state.is_active())
    }

    fn stopped_by_user(&self, svc: &Service) -> bool {
        self.runner
            .info(&run_key(svc))
            .is_some_and(|r| r.state == crate::run::RunState::Stopped)
    }

    fn check(&self, svc: &Service) -> Result<(), String> {
        // The orchestrator only asks services that have a check.
        svc.health
            .as_ref()
            .map_or(Ok(()), |h| check(h, CHECK_TIMEOUT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compose(file: &str, service: &str) -> Service {
        Service {
            id: "db".into(),
            name: "db".into(),
            order: 1,
            source: ServiceSource::Compose {
                project_id: ProjectId("p".into()),
                file: file.into(),
                service: service.into(),
            },
            port: Some(5432),
            health: None,
            depends_on: vec![],
        }
    }

    #[test]
    fn compose_lines_quote_the_file() {
        assert_eq!(
            compose_up_line("infra/docker-compose.yml", "db"),
            "docker compose -f \"infra/docker-compose.yml\" up db"
        );
        assert_eq!(
            compose_stop_line("compose.yaml", "cache"),
            "docker compose -f \"compose.yaml\" stop cache"
        );
    }

    #[test]
    fn run_keys_separate_compose_worktree_and_plain_runs() {
        assert_eq!(
            run_key(&compose("compose.yaml", "db")).command_id.0,
            "compose:compose.yaml:db"
        );
        let mut s = compose("x", "y");
        s.source = ServiceSource::Command {
            project_id: ProjectId("p".into()),
            command_id: CommandId("script:dev".into()),
            worktree: None,
        };
        assert_eq!(run_key(&s).command_id.0, "script:dev");
        s.source = ServiceSource::Command {
            project_id: ProjectId("p".into()),
            command_id: CommandId("script:dev".into()),
            worktree: Some(PathBuf::from(r"Q:\dev\shop-hotfix")),
        };
        assert!(run_key(&s).command_id.0.starts_with("script:dev@"));
    }

    #[test]
    fn worktree_rebase_keeps_the_subfolder() {
        let got = rebase_into_worktree(
            Path::new(r"Q:\dev\shop\apps\web"),
            Path::new(r"Q:\dev\shop"),
            Path::new(r"Q:\dev\shop-hotfix"),
        );
        assert_eq!(got, PathBuf::from(r"Q:\dev\shop-hotfix\apps\web"));
    }
}
