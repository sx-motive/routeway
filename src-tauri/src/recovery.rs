use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

const RESTART_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Session {
    generation: u64,
    active: bool,
    last_restart: Option<Instant>,
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
        session.last_restart = None;
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

    pub fn eligible(&self, token: u64) -> bool {
        let session = self.session.lock().unwrap();
        session.active
            && session.generation == token
            && session.last_restart.is_none_or(|at| at.elapsed() >= RESTART_COOLDOWN)
    }

    fn reserve(&self, token: u64) -> bool {
        if !self.eligible(token) {
            return false;
        }
        self.session.lock().unwrap().last_restart = Some(Instant::now());
        true
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

/// Restarts the proxy-mode core after its process exited on its own. Failed
/// probes never trigger this: a restart cannot fix an unreachable server.
pub fn restart_exited_core(app: AppHandle) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(restart_exited_core_inner(app))
}

async fn restart_exited_core_inner(app: AppHandle) {
    let state = app.state::<RecoveryState>();
    let Some(token) = state.token() else { return };
    let settings = crate::settings::load(&app);
    let connection = app.state::<crate::ConnectionState>();
    let _guard = connection.0.lock().await;
    let core = app.state::<crate::core::CoreState>();
    if settings.tun || core.is_running() || !state.reserve(token) {
        return;
    }
    core.recovery_log(&app, t!("recovery.restarting"));
    let result = match crate::core::restart_keeping_log(&app, &settings).await {
        Ok(()) => crate::reconcile_system_proxy(&app, &settings).await,
        Err(err) => Err(err),
    };
    if !state.valid(token) {
        return;
    }
    match result {
        Ok(()) => core.recovery_log(&app, t!("recovery.started")),
        Err(err) => {
            core.recovery_log(&app, t!("recovery.failed", err = err));
            crate::release_system_proxy(&app, settings.listen_port).await;
        }
    }
    let _ = app.emit("connection-changed", ());
    crate::health::publish(app.app_handle());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_only_restarts_after_explicit_activation() {
        let state = RecoveryState::default();
        let token = state.begin();
        assert!(!state.eligible(token));
        assert!(state.activate(token));
        assert!(state.reserve(token));
        assert!(!state.reserve(token));
        let next = state.begin();
        assert!(!state.eligible(token));
        assert!(state.activate(next));
        assert!(state.eligible(next));
    }

    #[test]
    fn manual_stop_invalidates_the_session() {
        let state = RecoveryState::default();
        let token = state.begin();
        assert!(state.activate(token));
        state.begin();
        assert!(!state.valid(token));
        assert!(!state.activate(token));
        assert!(!state.reserve(token));
    }
}
