//! Connection intent and a bounded recovery policy. The existing health task
//! drives it; no additional polling task or process scanner is needed.
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Healthy,
    Checking,
    Recovering,
    WaitingNetwork,
    Unavailable,
}

#[derive(Default)]
struct Policy {
    failures: u32,
    consecutive_failures: u32,
    successes: u32,
    attempts: VecDeque<Instant>,
    phase: Phase,
}

impl Policy {
    fn observe(&mut self, success: bool, missing: bool) -> bool {
        if success {
            self.consecutive_failures = 0;
            self.successes = self.successes.saturating_add(1);
            if self.successes >= 2 {
                self.failures = 0;
                self.phase = Phase::Healthy;
            }
            return false;
        }
        self.successes = 0;
        self.consecutive_failures =
            self.consecutive_failures
                .saturating_add(1)
                .max(if missing { 3 } else { 0 });
        self.failures = self
            .failures
            .saturating_add(1)
            .max(if missing { 3 } else { 0 });
        self.phase = if self.consecutive_failures < 3 {
            Phase::Checking
        } else if self.phase == Phase::WaitingNetwork {
            Phase::WaitingNetwork
        } else {
            Phase::Unavailable
        };
        self.consecutive_failures >= 3
    }

    fn can_restart(&mut self, now: Instant) -> bool {
        while self
            .attempts
            .front()
            .is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(600))
        {
            self.attempts.pop_front();
        }
        self.attempts.len() < 2
            && self
                .attempts
                .back()
                .is_none_or(|at| now.duration_since(*at) >= Duration::from_secs(60))
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(if self.failures == 0 {
            10
        } else if self.attempts.len() >= 2 {
            60
        } else {
            match self.failures {
                0..=3 => 10,
                4..=5 => 20,
                6..=8 => 30,
                _ => 60,
            }
        })
    }
}

#[derive(Default)]
struct Session {
    generation: u64,
    active: bool,
    policy: Policy,
}

#[derive(Default)]
pub struct RecoveryState {
    session: Mutex<Session>,
    pub wake: Notify,
}

impl RecoveryState {
    /// Invalidate in-flight samples before waiting for the connection lock.
    pub fn begin(&self) -> u64 {
        let mut session = self.session.lock().unwrap();
        session.generation = session.generation.wrapping_add(1);
        session.active = false;
        session.policy = Policy::default();
        self.wake.notify_one();
        session.generation
    }

    pub fn current(&self, token: u64) -> bool {
        self.session.lock().unwrap().generation == token
    }

    pub fn activate(&self, token: u64) -> bool {
        let mut session = self.session.lock().unwrap();
        if session.generation != token {
            return false;
        }
        session.active = true;
        session.policy.phase = Phase::Healthy;
        session.policy.successes = 2;
        self.wake.notify_one();
        true
    }

    pub fn token(&self) -> Option<u64> {
        let session = self.session.lock().unwrap();
        session.active.then_some(session.generation)
    }

    pub fn valid(&self, token: u64) -> bool {
        self.token() == Some(token)
    }

    pub fn phase(&self) -> Phase {
        self.session.lock().unwrap().policy.phase
    }

    pub fn interval(&self) -> Duration {
        self.session.lock().unwrap().policy.interval()
    }

    pub fn observe(&self, token: u64, success: bool, missing: bool) -> bool {
        let mut session = self.session.lock().unwrap();
        if !session.active || session.generation != token {
            return false;
        }
        session.policy.observe(success, missing)
    }

    pub fn eligible(&self, token: u64) -> bool {
        let mut session = self.session.lock().unwrap();
        session.active && session.generation == token && session.policy.can_restart(Instant::now())
    }

    pub fn reserve(&self, token: u64) -> bool {
        let mut session = self.session.lock().unwrap();
        let now = Instant::now();
        if !session.active || session.generation != token || !session.policy.can_restart(now) {
            return false;
        }
        session.policy.attempts.push_back(now);
        session.policy.phase = Phase::Recovering;
        true
    }

    pub fn set_phase(&self, token: u64, phase: Phase) {
        let mut session = self.session.lock().unwrap();
        if session.active && session.generation == token {
            session.policy.phase = phase;
        }
    }
}

pub fn cancel(app: &AppHandle) -> u64 {
    let token = app.state::<RecoveryState>().begin();
    crate::health::publish(app);
    token
}

pub fn activate(app: &AppHandle, token: u64) -> bool {
    let active = app.state::<RecoveryState>().activate(token);
    crate::health::publish(app);
    active
}

/// Called only after repeated failed tunnel probes, or an exited core.
pub async fn recover(app: &AppHandle, settings: &crate::settings::Settings, token: u64) {
    let state = app.state::<RecoveryState>();
    if !state.eligible(token) {
        return;
    }
    let missing = !app.state::<crate::core::CoreState>().is_running();
    let listener_ready =
        !missing && crate::health::local_listener_available(settings.listen_port).await;
    // no_proxy bypasses the OS proxy in proxy mode. Under TUN it would still
    // traverse the adapter and cannot establish independent network health.
    if listener_ready && !settings.tun && !crate::health::direct_network_available().await {
        let previous = state.phase();
        state.set_phase(token, Phase::WaitingNetwork);
        if state.valid(token) && previous != Phase::WaitingNetwork {
            app.state::<crate::core::CoreState>()
                .recovery_log(app, t!("recovery.network_wait"));
        }
        crate::health::publish(app);
        return;
    }
    let connection = app.state::<crate::ConnectionState>();
    let _guard = connection.0.lock().await;
    if !state.valid(token) {
        return;
    }
    // Never generate automatic elevation prompts after the elevated TUN host dies.
    if settings.tun && !app.state::<crate::core::CoreState>().is_tun() {
        state.set_phase(token, Phase::Unavailable);
        crate::health::publish(app);
        return;
    }
    if !state.reserve(token) {
        return;
    }
    crate::health::publish(app);
    let core = app.state::<crate::core::CoreState>();
    core.recovery_log(app, t!("recovery.restarting"));
    let result = crate::core::restart_for_recovery(app, settings).await;
    if !state.valid(token) {
        return;
    } // a queued manual stop owns cleanup now
    if let Err(err) = result {
        core.recovery_log(app, t!("recovery.failed", err = err));
        state.set_phase(token, Phase::Unavailable);
        if !core.is_running() {
            crate::release_system_proxy(app, settings.listen_port).await;
        }
    } else {
        // Keep a usable local core even if REALITY is still unavailable. Direct
        // rules remain direct and proxy rules never silently fall back to direct.
        match crate::reconcile_system_proxy(app, settings).await {
            Ok(()) => {
                state.set_phase(token, Phase::Checking);
                core.recovery_log(app, t!("recovery.started"));
            }
            Err(err) => {
                state.set_phase(token, Phase::Unavailable);
                core.recovery_log(app, t!("recovery.failed", err = err));
            }
        }
    }
    let _ = app.emit("connection-changed", ());
    crate::health::publish(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_failures_are_required_but_an_exited_core_is_immediate() {
        let mut policy = Policy::default();
        assert!(!policy.observe(false, false));
        assert!(!policy.observe(false, false));
        assert!(policy.observe(false, false));
        assert!(Policy::default().observe(false, true));
    }

    #[test]
    fn intermittent_success_breaks_the_failure_streak() {
        let mut policy = Policy::default();
        for _ in 0..5 {
            assert!(!policy.observe(false, false));
            assert!(!policy.observe(true, false));
        }
        assert!(!policy.observe(false, false));
        assert!(!policy.observe(false, false));
        assert!(policy.observe(false, false));
    }

    #[test]
    fn two_successes_confirm_recovery_without_resetting_restart_budget() {
        let mut policy = Policy::default();
        policy.attempts.push_back(Instant::now());
        for _ in 0..3 {
            policy.observe(false, false);
        }
        policy.observe(true, false);
        assert_eq!(policy.failures, 3);
        policy.observe(false, false);
        policy.observe(true, false);
        assert_eq!(policy.failures, 4);
        policy.observe(true, false);
        assert_eq!(policy.failures, 0);
        assert_eq!(policy.phase, Phase::Healthy);
        assert_eq!(policy.attempts.len(), 1);
    }

    #[test]
    fn restarts_have_a_cooldown_and_a_rolling_ten_minute_budget() {
        let now = Instant::now();
        let mut policy = Policy::default();
        assert!(policy.can_restart(now));
        policy.attempts.push_back(now);
        assert!(!policy.can_restart(now + Duration::from_secs(59)));
        assert!(policy.can_restart(now + Duration::from_secs(60)));
        policy.attempts.push_back(now + Duration::from_secs(60));
        assert!(!policy.can_restart(now + Duration::from_secs(599)));
        assert!(policy.can_restart(now + Duration::from_secs(600)));
        assert_eq!(policy.attempts.len(), 1);
    }

    #[test]
    fn outage_polling_slows_down_to_one_minute() {
        let mut policy = Policy::default();
        assert_eq!(policy.interval(), Duration::from_secs(10));
        for _ in 0..4 {
            policy.observe(false, false);
        }
        assert_eq!(policy.interval(), Duration::from_secs(20));
        for _ in 0..2 {
            policy.observe(false, false);
        }
        assert_eq!(policy.interval(), Duration::from_secs(30));
        for _ in 0..3 {
            policy.observe(false, false);
        }
        assert_eq!(policy.interval(), Duration::from_secs(60));
    }

    #[test]
    fn healthy_polling_stays_fast_even_when_restart_budget_is_spent() {
        let mut policy = Policy::default();
        policy.attempts.extend([Instant::now(), Instant::now()]);
        for _ in 0..3 {
            policy.observe(false, false);
        }
        assert_eq!(policy.interval(), Duration::from_secs(60));
        policy.observe(true, false);
        policy.observe(true, false);
        assert_eq!(policy.interval(), Duration::from_secs(10));
        assert!(!policy.can_restart(Instant::now()));
    }

    #[test]
    fn waiting_for_network_does_not_reset_the_backoff_or_spend_restart_budget() {
        let mut policy = Policy::default();
        for _ in 0..3 {
            policy.observe(false, false);
        }
        policy.phase = Phase::WaitingNetwork;
        for _ in 0..6 {
            policy.observe(false, false);
        }
        assert_eq!(policy.phase, Phase::WaitingNetwork);
        assert_eq!(policy.interval(), Duration::from_secs(60));
        assert!(policy.attempts.is_empty());
        assert!(policy.can_restart(Instant::now()));
    }

    #[test]
    fn a_session_only_recovers_after_explicit_activation() {
        let state = RecoveryState::default();
        let token = state.begin();
        assert!(!state.eligible(token));
        assert!(!state.reserve(token));
        assert!(state.activate(token));
        assert!(state.reserve(token));
        assert!(!state.reserve(token));
        assert_eq!(state.phase(), Phase::Recovering);
        let next = state.begin();
        assert!(!state.eligible(token));
        assert!(state.activate(next));
        assert!(state.eligible(next));
    }

    #[test]
    fn manual_stop_invalidates_pending_samples_restarts_and_activation() {
        let state = RecoveryState::default();
        let token = state.begin();
        assert!(state.activate(token));
        state.begin();
        assert!(!state.valid(token));
        assert!(!state.activate(token));
        assert!(!state.reserve(token));
        assert!(!state.observe(token, false, true));
        assert_eq!(state.phase(), Phase::Idle);
    }
}
