//! Starting and supervising a workspace (T7.4).
//!
//! A pure state machine: `tick(now, driver)` looks at the services through a
//! [`ServiceDriver`] and moves one step. A thread calls it a few times a second in the
//! app; tests call it with a fake driver and a fake clock, so timeouts and crash
//! loops are tested in microseconds.
//!
//! * Start: service *n* must be healthy (or, without a check, up for a short grace
//!   period) before *n+1* starts. Not healthy within the timeout = the workspace fails
//!   and later services are not started.
//! * Running: crashed services are restarted under the restart policy; checks run
//!   every interval and 3 misses mark a service unhealthy.
//! * Stop: services stop in reverse order, each waited for (with a timeout).

use serde::Serialize;

use super::health::Misses;
use super::model::{RestartPolicy, Service, Workspace};
use super::order::{OrderError, start_order};

/// How the orchestrator sees services. The app implements it on top of `Runner`.
pub trait ServiceDriver {
    fn start(&self, svc: &Service) -> Result<(), String>;
    fn stop(&self, svc: &Service);
    fn is_running(&self, svc: &Service) -> bool;
    /// Only called for services that have a health check.
    fn check(&self, svc: &Service) -> Result<(), String>;
    /// The user stopped it outside the workspace (Run tab, Running screen): that's
    /// not a crash, so it must not be restarted.
    fn stopped_by_user(&self, _svc: &Service) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// A starting service must be healthy within this.
    pub start_timeout_ms: u64,
    /// How often to check while waiting for a service to become healthy.
    pub poll_ms: u64,
    /// How often to check a running service (T7.3: 10 s).
    pub check_every_ms: u64,
    /// Without a health check, "up" means still running after this long.
    pub grace_ms: u64,
    pub stop_timeout_ms: u64,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            start_timeout_ms: 60_000,
            poll_ms: 1_000,
            check_every_ms: 10_000,
            grace_ms: 1_500,
            stop_timeout_ms: 15_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "phase",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Phase {
    Idle,
    /// Waiting for the service at `step` (start order) to become ready.
    Starting {
        step: usize,
    },
    Running,
    Stopping,
    Stopped,
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SvcState {
    /// Not started (yet).
    Pending,
    Starting,
    /// Running and its health check passes.
    Healthy,
    /// Running; it has no health check.
    Up,
    /// Running, but the check failed 3 times in a row.
    Unhealthy,
    /// Crashed more often than the restart policy allows; left stopped.
    GaveUp,
    Failed,
    Stopping,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SvcRuntime {
    pub id: String,
    pub state: SvcState,
    pub started_at: Option<u64>,
    pub last_check: Option<u64>,
    /// Restart times (ms) within the current policy window and before.
    pub restarts: Vec<u64>,
    /// Latest explanation: a failed check, why it gave up, etc.
    pub message: Option<String>,
    #[serde(skip)]
    misses: Misses,
    #[serde(skip)]
    stop_asked_at: Option<u64>,
}

impl SvcRuntime {
    fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            state: SvcState::Pending,
            started_at: None,
            last_check: None,
            restarts: Vec::new(),
            message: None,
            misses: Misses::default(),
            stop_asked_at: None,
        }
    }

    fn is_live(&self) -> bool {
        matches!(
            self.state,
            SvcState::Starting | SvcState::Healthy | SvcState::Up | SvcState::Unhealthy
        )
    }
}

#[derive(Debug, Clone)]
pub struct Orchestrator {
    /// Services in start order.
    services: Vec<Service>,
    policy: RestartPolicy,
    timing: Timing,
    pub phase: Phase,
    /// Same order as `services`.
    pub svcs: Vec<SvcRuntime>,
}

/// What the UI shows for a running workspace.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorView {
    #[serde(flatten)]
    pub phase: Phase,
    pub services: Vec<SvcRuntime>,
}

impl Orchestrator {
    pub fn new(ws: &Workspace, timing: Timing) -> Result<Self, OrderError> {
        let order = start_order(&ws.services)?;
        let services: Vec<Service> = order.into_iter().map(|i| ws.services[i].clone()).collect();
        Ok(Self {
            svcs: services.iter().map(|s| SvcRuntime::new(&s.id)).collect(),
            services,
            policy: ws.restart,
            timing,
            phase: Phase::Idle,
        })
    }

    pub fn view(&self) -> OrchestratorView {
        OrchestratorView {
            phase: self.phase.clone(),
            services: self.svcs.clone(),
        }
    }

    pub fn services(&self) -> &[Service] {
        &self.services
    }

    /// Nothing is starting, running or stopping any more.
    pub fn is_settled(&self) -> bool {
        matches!(self.phase, Phase::Idle | Phase::Stopped)
            || (matches!(self.phase, Phase::Failed { .. })
                && !self.svcs.iter().any(SvcRuntime::is_live))
    }

    pub fn start(&mut self) {
        if matches!(
            self.phase,
            Phase::Starting { .. } | Phase::Running | Phase::Stopping
        ) {
            return;
        }
        for (rt, svc) in self.svcs.iter_mut().zip(&self.services) {
            *rt = SvcRuntime::new(&svc.id);
        }
        self.phase = if self.services.is_empty() {
            Phase::Running
        } else {
            Phase::Starting { step: 0 }
        };
    }

    pub fn request_stop(&mut self) {
        if !matches!(self.phase, Phase::Idle | Phase::Stopped) {
            self.phase = Phase::Stopping;
        }
    }

    /// Advances one step. Returns true if anything the UI shows changed.
    pub fn tick(&mut self, now: u64, d: &dyn ServiceDriver) -> bool {
        let before = self.fingerprint();
        match self.phase.clone() {
            Phase::Starting { step } => {
                self.monitor(now, d, step);
                self.advance_start(now, d, step);
            }
            Phase::Running | Phase::Failed { .. } => {
                let all = self.services.len();
                self.monitor(now, d, all);
            }
            Phase::Stopping => self.stop_step(now, d),
            Phase::Idle | Phase::Stopped => {}
        }
        self.fingerprint() != before
    }

    fn fingerprint(&self) -> String {
        format!("{:?}", self.view())
    }

    fn fail(&mut self, i: usize, message: String) {
        let name = self.services[i].name.clone();
        self.svcs[i].state = SvcState::Failed;
        self.svcs[i].message = Some(message.clone());
        self.phase = Phase::Failed {
            reason: format!("{name}: {message}"),
        };
    }

    fn launch(&mut self, i: usize, now: u64, d: &dyn ServiceDriver) -> Result<(), String> {
        d.start(&self.services[i])?;
        let rt = &mut self.svcs[i];
        rt.state = SvcState::Starting;
        rt.started_at = Some(now);
        rt.last_check = None;
        rt.misses = Misses::default();
        rt.stop_asked_at = None;
        Ok(())
    }

    /// Is a starting service ready? Runs at most one check per `poll_ms`.
    fn ready(&mut self, i: usize, now: u64, d: &dyn ServiceDriver) -> bool {
        let svc = &self.services[i];
        let rt = &mut self.svcs[i];
        let started = rt.started_at.unwrap_or(now);
        if svc.health.is_none() {
            return now.saturating_sub(started) >= self.timing.grace_ms;
        }
        let due = rt
            .last_check
            .is_none_or(|t| now.saturating_sub(t) >= self.timing.poll_ms);
        if !due {
            return false;
        }
        rt.last_check = Some(now);
        match d.check(svc) {
            Ok(()) => true,
            Err(e) => {
                rt.message = Some(e);
                false
            }
        }
    }

    fn mark_ready(&mut self, i: usize) {
        let rt = &mut self.svcs[i];
        rt.state = if self.services[i].health.is_some() {
            SvcState::Healthy
        } else {
            SvcState::Up
        };
        rt.message = None;
    }

    fn timed_out(&self, i: usize, now: u64) -> bool {
        self.svcs[i]
            .started_at
            .is_some_and(|t| now.saturating_sub(t) > self.timing.start_timeout_ms)
    }

    fn advance_start(&mut self, now: u64, d: &dyn ServiceDriver, step: usize) {
        match self.svcs[step].state {
            SvcState::Pending => {
                if let Err(e) = self.launch(step, now, d) {
                    self.fail(step, format!("couldn't start: {e}"));
                }
            }
            SvcState::Starting => {
                if !d.is_running(&self.services[step]) {
                    self.fail(step, "exited before it was ready".into());
                } else if self.ready(step, now, d) {
                    self.mark_ready(step);
                    self.phase = if step + 1 == self.services.len() {
                        Phase::Running
                    } else {
                        Phase::Starting { step: step + 1 }
                    };
                } else if self.timed_out(step, now) {
                    let why = self.svcs[step].message.clone().unwrap_or_default();
                    let secs = self.timing.start_timeout_ms / 1000;
                    self.fail(step, format!("not ready after {secs} s ({why})"));
                }
            }
            _ => {}
        }
    }

    /// Watches services `0..upto`: crashes, restarts finishing, periodic checks.
    fn monitor(&mut self, now: u64, d: &dyn ServiceDriver, upto: usize) {
        for i in 0..upto {
            let state = self.svcs[i].state;
            match state {
                SvcState::Healthy | SvcState::Up | SvcState::Unhealthy => {
                    if !d.is_running(&self.services[i]) {
                        if d.stopped_by_user(&self.services[i]) {
                            self.svcs[i].state = SvcState::Stopped;
                            self.svcs[i].message = Some("stopped outside the workspace".into());
                        } else {
                            self.crashed(i, now, d);
                        }
                        continue;
                    }
                    let has_check = self.services[i].health.is_some();
                    let rt = &mut self.svcs[i];
                    let due = rt
                        .last_check
                        .is_none_or(|t| now.saturating_sub(t) >= self.timing.check_every_ms);
                    if has_check && due {
                        rt.last_check = Some(now);
                        match d.check(&self.services[i]) {
                            Ok(()) => {
                                rt.misses.record(true);
                                rt.state = SvcState::Healthy;
                                rt.message = None;
                            }
                            Err(e) => {
                                if !rt.misses.record(false) {
                                    rt.state = SvcState::Unhealthy;
                                }
                                rt.message = Some(e);
                            }
                        }
                    }
                }
                // A restart in progress (the first start is handled by `advance_start`).
                SvcState::Starting if !self.svcs[i].restarts.is_empty() => {
                    if !d.is_running(&self.services[i]) {
                        self.crashed(i, now, d);
                    } else if self.ready(i, now, d) {
                        self.mark_ready(i);
                    } else if self.timed_out(i, now) {
                        self.svcs[i].state = SvcState::Unhealthy;
                    }
                }
                _ => {}
            }
        }
    }

    fn crashed(&mut self, i: usize, now: u64, d: &dyn ServiceDriver) {
        let window_ms = self.policy.window_secs.saturating_mul(1000);
        let recent = self.svcs[i]
            .restarts
            .iter()
            .filter(|&&t| now.saturating_sub(t) < window_ms)
            .count() as u32;
        if !self.policy.enabled {
            self.svcs[i].state = SvcState::GaveUp;
            self.svcs[i].message = Some("exited; automatic restarts are off".into());
            return;
        }
        if recent >= self.policy.max {
            self.svcs[i].state = SvcState::GaveUp;
            self.svcs[i].message = Some(format!(
                "crashed {} times within {} s; not restarting it again",
                recent + 1,
                self.policy.window_secs
            ));
            return;
        }
        self.svcs[i].restarts.push(now);
        let n = recent + 1;
        match self.launch(i, now, d) {
            Ok(()) => {
                self.svcs[i].message = Some(format!(
                    "restarted after a crash ({n} of {})",
                    self.policy.max
                ));
            }
            Err(e) => {
                self.svcs[i].state = SvcState::Failed;
                self.svcs[i].message = Some(format!("couldn't restart: {e}"));
            }
        }
    }

    /// Stops the highest-ordered live service, waits for it, then the next one down.
    fn stop_step(&mut self, now: u64, d: &dyn ServiceDriver) {
        for i in (0..self.services.len()).rev() {
            let rt = &self.svcs[i];
            if matches!(rt.state, SvcState::Pending | SvcState::Stopped) {
                continue;
            }
            let svc = &self.services[i];
            let running = d.is_running(svc);
            let rt = &mut self.svcs[i];
            match rt.stop_asked_at {
                None if !running => {
                    rt.state = SvcState::Stopped;
                }
                None => {
                    d.stop(svc);
                    rt.state = SvcState::Stopping;
                    rt.stop_asked_at = Some(now);
                    return;
                }
                Some(_) if !running => {
                    rt.state = SvcState::Stopped;
                }
                Some(t) if now.saturating_sub(t) > self.timing.stop_timeout_ms => {
                    rt.state = SvcState::Stopped;
                    rt.message = Some("didn't confirm it stopped in time".into());
                }
                Some(_) => return,
            }
        }
        self.phase = Phase::Stopped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectId;
    use crate::run::CommandId;
    use crate::workspace::model::{Health, ServiceSource, WorkspaceId};
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    /// A fake service: becomes healthy `healthy_after` ms after each start (never if
    /// `None`), crashes `crash_after` ms after each start, stops on request.
    #[derive(Default, Clone)]
    struct FakeSvc {
        running: bool,
        started_at: u64,
        healthy_after: Option<u64>,
        crash_after: Option<u64>,
        refuse_start: bool,
        user_stopped: bool,
    }

    #[derive(Default)]
    struct Fake {
        now: Cell<u64>,
        svcs: RefCell<HashMap<String, FakeSvc>>,
        log: RefCell<Vec<String>>,
    }

    impl Fake {
        fn with(specs: &[(&str, Option<u64>, Option<u64>)]) -> Self {
            let f = Fake::default();
            for (id, healthy, crash) in specs {
                f.svcs.borrow_mut().insert(
                    (*id).into(),
                    FakeSvc {
                        healthy_after: *healthy,
                        crash_after: *crash,
                        ..FakeSvc::default()
                    },
                );
            }
            f
        }
        fn alive(&self, s: &FakeSvc) -> bool {
            s.running
                && s.crash_after
                    .is_none_or(|c| self.now.get() < s.started_at + c)
        }
        fn starts(&self, id: &str) -> usize {
            self.log
                .borrow()
                .iter()
                .filter(|l| *l == &format!("start {id}"))
                .count()
        }
    }

    impl ServiceDriver for Fake {
        fn start(&self, svc: &Service) -> Result<(), String> {
            let mut all = self.svcs.borrow_mut();
            let s = all.get_mut(&svc.id).unwrap();
            if s.refuse_start {
                return Err("program not found".into());
            }
            s.running = true;
            s.started_at = self.now.get();
            self.log.borrow_mut().push(format!("start {}", svc.id));
            Ok(())
        }
        fn stop(&self, svc: &Service) {
            self.svcs.borrow_mut().get_mut(&svc.id).unwrap().running = false;
            self.log.borrow_mut().push(format!("stop {}", svc.id));
        }
        fn is_running(&self, svc: &Service) -> bool {
            self.alive(&self.svcs.borrow()[&svc.id])
        }
        fn check(&self, svc: &Service) -> Result<(), String> {
            let all = self.svcs.borrow();
            let s = &all[&svc.id];
            match s.healthy_after {
                Some(h) if self.alive(s) && self.now.get() >= s.started_at + h => Ok(()),
                _ => Err("nothing on the port yet".into()),
            }
        }
        fn stopped_by_user(&self, svc: &Service) -> bool {
            self.svcs.borrow()[&svc.id].user_stopped
        }
    }

    fn svc(id: &str, order: u32, health: bool) -> Service {
        Service {
            id: id.into(),
            name: id.into(),
            order,
            source: ServiceSource::Command {
                project_id: ProjectId("p".into()),
                command_id: CommandId(format!("script:{id}")),
                worktree: None,
            },
            port: None,
            health: health.then_some(Health::Tcp { port: 1 }),
            depends_on: vec![],
        }
    }

    fn ws(services: Vec<Service>) -> Workspace {
        Workspace {
            id: WorkspaceId("w".into()),
            name: "Wallet".into(),
            services,
            restart: RestartPolicy::default(),
            after_start: Default::default(),
        }
    }

    /// Ticks every 250 ms until `until` (ms) or the predicate holds.
    fn run(o: &mut Orchestrator, f: &Fake, until: u64, stop_when: impl Fn(&Orchestrator) -> bool) {
        while f.now.get() <= until {
            o.tick(f.now.get(), f);
            if stop_when(o) {
                return;
            }
            f.now.set(f.now.get() + 250);
        }
    }

    fn state(o: &Orchestrator, id: &str) -> SvcState {
        o.svcs.iter().find(|s| s.id == id).unwrap().state
    }

    #[test]
    fn happy_path_starts_each_after_the_previous_is_healthy() {
        let f = Fake::with(&[
            ("db", Some(2_000), None),
            ("api", Some(500), None),
            ("web", None, None),
        ]);
        let mut o = Orchestrator::new(
            &ws(vec![
                svc("web", 3, false),
                svc("db", 1, true),
                svc("api", 2, true),
            ]),
            Timing::default(),
        )
        .unwrap();
        o.start();

        run(&mut o, &f, 1_000, |_| false);
        // db isn't healthy yet, so api must not have started.
        assert_eq!(f.starts("db"), 1);
        assert_eq!(f.starts("api"), 0);

        run(&mut o, &f, 20_000, |o| o.phase == Phase::Running);
        assert_eq!(o.phase, Phase::Running);
        assert_eq!(*f.log.borrow(), vec!["start db", "start api", "start web"]);
        assert_eq!(state(&o, "db"), SvcState::Healthy);
        assert_eq!(state(&o, "web"), SvcState::Up);
    }

    #[test]
    fn slow_start_times_out_and_stops_the_sequence() {
        let f = Fake::with(&[("db", None, None), ("api", Some(0), None)]);
        let mut o = Orchestrator::new(
            &ws(vec![svc("db", 1, true), svc("api", 2, true)]),
            Timing::default(),
        )
        .unwrap();
        o.start();
        run(&mut o, &f, 120_000, |o| {
            matches!(o.phase, Phase::Failed { .. })
        });
        let Phase::Failed { reason } = &o.phase else {
            panic!("{:?}", o.phase)
        };
        assert!(reason.contains("not ready after 60 s"), "{reason}");
        assert!(reason.contains("nothing on the port"), "{reason}");
        assert!(f.now.get() >= 60_000 && f.now.get() < 62_000);
        assert_eq!(f.starts("api"), 0);
        assert_eq!(state(&o, "db"), SvcState::Failed);
    }

    #[test]
    fn crash_loop_is_restarted_up_to_the_limit_then_left() {
        // Healthy quickly, then crashes 5 s after every start.
        let f = Fake::with(&[("api", Some(0), Some(5_000))]);
        let mut o = Orchestrator::new(&ws(vec![svc("api", 1, true)]), Timing::default()).unwrap();
        o.start();
        run(&mut o, &f, 200_000, |o| state(o, "api") == SvcState::GaveUp);
        assert_eq!(state(&o, "api"), SvcState::GaveUp);
        assert_eq!(f.starts("api"), 4, "first start + 3 restarts");
        let msg = o.svcs[0].message.clone().unwrap();
        assert!(msg.contains("crashed 4 times within 300 s"), "{msg}");
        // Nothing more happens afterwards.
        let starts = f.starts("api");
        run(&mut o, &f, 260_000, |_| false);
        assert_eq!(f.starts("api"), starts);
    }

    #[test]
    fn crashes_spread_beyond_the_window_keep_being_restarted() {
        let f = Fake::with(&[("api", Some(0), Some(200_000))]);
        let mut w = ws(vec![svc("api", 1, true)]);
        w.restart = RestartPolicy {
            enabled: true,
            max: 1,
            window_secs: 100,
        };
        let mut o = Orchestrator::new(&w, Timing::default()).unwrap();
        o.start();
        run(&mut o, &f, 1_000_000, |_| false);
        assert!(f.starts("api") >= 4, "restarted {} times", f.starts("api"));
        assert_ne!(state(&o, "api"), SvcState::GaveUp);
    }

    #[test]
    fn stop_while_starting_stops_in_reverse_and_skips_the_rest() {
        let f = Fake::with(&[
            ("db", Some(0), None),
            ("api", None, None),
            ("web", Some(0), None),
        ]);
        let mut o = Orchestrator::new(
            &ws(vec![
                svc("db", 1, true),
                svc("api", 2, true),
                svc("web", 3, true),
            ]),
            Timing::default(),
        )
        .unwrap();
        o.start();
        run(&mut o, &f, 5_000, |_| false); // db healthy, api still starting
        assert_eq!(o.phase, Phase::Starting { step: 1 });

        o.request_stop();
        run(&mut o, &f, 30_000, |o| o.phase == Phase::Stopped);
        assert_eq!(o.phase, Phase::Stopped);
        assert_eq!(
            *f.log.borrow(),
            vec!["start db", "start api", "stop api", "stop db"]
        );
        assert_eq!(state(&o, "web"), SvcState::Pending, "never started");
        assert!(o.is_settled());
    }

    #[test]
    fn exit_during_start_and_refused_start_fail_clearly() {
        let f = Fake::with(&[("db", None, Some(300))]);
        let mut o = Orchestrator::new(&ws(vec![svc("db", 1, true)]), Timing::default()).unwrap();
        o.start();
        run(&mut o, &f, 5_000, |o| {
            matches!(o.phase, Phase::Failed { .. })
        });
        assert!(
            matches!(&o.phase, Phase::Failed { reason } if reason.contains("exited before it was ready"))
        );

        let f = Fake::with(&[("db", Some(0), None)]);
        f.svcs.borrow_mut().get_mut("db").unwrap().refuse_start = true;
        let mut o = Orchestrator::new(&ws(vec![svc("db", 1, true)]), Timing::default()).unwrap();
        o.start();
        o.tick(0, &f);
        assert!(
            matches!(&o.phase, Phase::Failed { reason } if reason.contains("program not found"))
        );
    }

    #[test]
    fn three_failed_checks_mark_unhealthy_without_restarting() {
        let f = Fake::with(&[("api", Some(0), None)]);
        let mut o = Orchestrator::new(&ws(vec![svc("api", 1, true)]), Timing::default()).unwrap();
        o.start();
        run(&mut o, &f, 2_000, |o| o.phase == Phase::Running);
        // The port goes away but the process keeps running.
        f.svcs.borrow_mut().get_mut("api").unwrap().healthy_after = None;
        run(&mut o, &f, 25_000, |_| false);
        assert_eq!(
            state(&o, "api"),
            SvcState::Healthy,
            "two misses are tolerated"
        );
        run(&mut o, &f, 40_000, |_| false);
        assert_eq!(state(&o, "api"), SvcState::Unhealthy);
        assert_eq!(f.starts("api"), 1);
    }

    #[test]
    fn a_user_stop_is_not_treated_as_a_crash() {
        let f = Fake::with(&[("api", Some(0), None)]);
        let mut o = Orchestrator::new(&ws(vec![svc("api", 1, true)]), Timing::default()).unwrap();
        o.start();
        run(&mut o, &f, 2_000, |o| o.phase == Phase::Running);
        {
            let mut all = f.svcs.borrow_mut();
            let s = all.get_mut("api").unwrap();
            s.running = false;
            s.user_stopped = true;
        }
        run(&mut o, &f, 10_000, |_| false);
        assert_eq!(state(&o, "api"), SvcState::Stopped);
        assert_eq!(f.starts("api"), 1, "not restarted");
    }

    #[test]
    fn empty_workspace_is_running_at_once() {
        let mut o = Orchestrator::new(&ws(vec![]), Timing::default()).unwrap();
        o.start();
        assert_eq!(o.phase, Phase::Running);
    }
}
