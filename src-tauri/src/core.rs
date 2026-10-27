//! Supervisor for the bundled sing-box sidecar.

use std::collections::VecDeque;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::corelog::{LogEvent, LogParser};
use crate::settings::Settings;
use crate::singbox;
use crate::tunhost::{self, HostPaths};

const LOG_CAPACITY: usize = 300;

enum Running {
    Sidecar(CommandChild),
    Tun { host_pid: u32 },
}

impl Running {
    fn pid(&self) -> u32 {
        match self {
            Running::Sidecar(child) => child.pid(),
            Running::Tun { host_pid } => *host_pid,
        }
    }
}

#[derive(Default)]
pub struct CoreState {
    running: Mutex<Option<Running>>,
    log: Mutex<VecDeque<LogEvent>>,
    parser: Mutex<LogParser>,
}

impl CoreState {
    pub fn connection_id(&self) -> Option<u32> {
        self.running.lock().ok()?.as_ref().map(Running::pid)
    }

    pub fn is_running(&self) -> bool {
        self.running.lock().map(|r| r.is_some()).unwrap_or(false)
    }

    pub fn is_tun(&self) -> bool {
        self.running
            .lock()
            .map(|r| matches!(*r, Some(Running::Tun { .. })))
            .unwrap_or(false)
    }

    pub fn log_events(&self) -> Vec<LogEvent> {
        self.log
            .lock()
            .map(|l| l.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn log_text(&self) -> String {
        self.log_events()
            .iter()
            .map(|e| e.raw.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn install(&self, running: Running) {
        if let Ok(mut slot) = self.running.lock() {
            *slot = Some(running);
        }
    }

    fn take(&self) -> Option<Running> {
        self.running.lock().ok().and_then(|mut slot| slot.take())
    }

    fn holds(&self, pid: u32) -> bool {
        self.running
            .lock()
            .map(|r| r.as_ref().map(Running::pid) == Some(pid))
            .unwrap_or(false)
    }

    /// An exit notice can arrive after a restart has already installed the next
    /// instance, so only the instance that actually exited may be cleared.
    fn release(&self, pid: u32) -> bool {
        let Ok(mut slot) = self.running.lock() else {
            return false;
        };
        if slot.as_ref().map(Running::pid) != Some(pid) {
            return false;
        }
        *slot = None;
        true
    }

    async fn unexpected_exit(app: AppHandle, pid: u32, reason: String) {
        let connection = app.state::<crate::ConnectionState>();
        let _guard = connection.0.lock().await;
        let state = app.state::<CoreState>();
        // A deliberate stop or an old process exiting after a restart must not
        // clear the new connection or restore its proxy configuration.
        if state.release(pid) {
            forget_pid(&app);
            let settings = crate::settings::load(&app);
            let recovery = app.state::<crate::recovery::RecoveryState>();
            let restart = !settings.tun && recovery.token().is_some_and(|token| recovery.eligible(token));
            if !restart {
                crate::release_system_proxy(&app, settings.listen_port).await;
            }
            if settings.tun && recovery.token().is_some() {
                state.recovery_log(&app, t!("recovery.tun_manual"));
            }
            state.record_exit(&app, reason);
            crate::health::publish(&app);
            if restart {
                tauri::async_runtime::spawn(crate::recovery::restart_exited_core(app.clone()));
            }
        }
    }

    fn clear_log(&self) {
        if let Ok(mut log) = self.log.lock() {
            log.clear();
        }
        if let Ok(mut parser) = self.parser.lock() {
            *parser = LogParser::default();
        }
    }

    fn record(&self, app: &AppHandle, line: &str) {
        let event = self.parser.lock().ok().and_then(|mut p| p.ingest(line));
        if let Some(event) = event {
            self.push_event(app, event);
        }
    }

    fn record_exit(&self, app: &AppHandle, reason: String) {
        self.push_event(app, LogEvent::stopped(reason.clone()));
        let _ = app.emit("core-exit", reason);
    }

    pub fn recovery_log(&self, app: &AppHandle, message: String) {
        let mut event = LogEvent::stopped(message);
        event.kind = crate::corelog::Kind::Other;
        event.level = crate::corelog::Level::Info;
        self.push_event(app, event);
    }

    fn push_event(&self, app: &AppHandle, event: LogEvent) {
        if let Ok(mut log) = self.log.lock() {
            if log.len() >= LOG_CAPACITY {
                log.pop_front();
            }
            log.push_back(event.clone());
        }
        let _ = app.emit("core-log", event);
    }
}

fn pid_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_path(app)?.with_file_name("core.pid"))
}

/// The core is a child process, but an app that is killed (rather than quit)
/// leaves it running and holding the port. Recording its pid lets the next
/// launch clean up after the previous one.
fn remember_pid(app: &AppHandle, pid: u32) {
    if let Ok(path) = pid_path(app) {
        let _ = fs::write(path, pid.to_string());
    }
}

fn forget_pid(app: &AppHandle) {
    if let Ok(path) = pid_path(app) {
        let _ = fs::remove_file(path);
    }
}

/// Kills a core left behind by a previous run, if the recorded pid still points
/// at one. Checking the process name first avoids killing an unrelated process
/// that happens to have inherited the pid.
pub fn reap_orphan(app: &AppHandle) {
    let Ok(path) = pid_path(app) else { return };
    let Ok(raw) = fs::read_to_string(&path) else {
        return;
    };
    let _ = fs::remove_file(&path);
    let Ok(pid) = raw.trim().parse::<u32>() else {
        return;
    };

    if !is_our_core(pid) {
        return;
    }

    #[cfg(windows)]
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output();

    #[cfg(not(windows))]
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();
}

fn is_our_core(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let Ok(out) = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
        else {
            return false;
        };
        String::from_utf8_lossy(&out.stdout).contains("sing-box")
    }
    #[cfg(not(windows))]
    {
        let Ok(out) = std::process::Command::new("ps")
            .args(["-o", "comm=", "-p", &pid.to_string()])
            .output()
        else {
            return false;
        };
        String::from_utf8_lossy(&out.stdout).contains("sing-box")
    }
}

/// True when nothing is listening on the port, i.e. we can claim it.
fn port_is_free(port: u16) -> bool {
    std::net::TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))).is_ok()
}

pub fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|err| t!("err.config_dir", err = err))?;
    fs::create_dir_all(&dir).map_err(|err| t!("err.create_dir", path = dir.display(), err = err))?;
    Ok(dir.join("sing-box.json"))
}

pub fn write_config(app: &AppHandle, settings: &Settings) -> Result<PathBuf, String> {
    let mut config = singbox::build_config(settings)?;
    let path = config_path(app)?;
    config["experimental"] = serde_json::json!({
        "cache_file": {
            "enabled": true,
            "path": path.with_file_name("cache.db"),
            "store_dns": true
        }
    });
    let body = serde_json::to_string_pretty(&config)
        .map_err(|err| t!("err.serialize_config", err = err))?;
    fs::write(&path, body).map_err(|err| t!("err.write_config", err = err))?;
    Ok(path)
}

fn tun_paths(app: &AppHandle) -> Result<HostPaths, String> {
    let config = config_path(app)?;
    let dir = config
        .parent()
        .ok_or_else(|| t!("err.config_dir_plain"))?;
    Ok(HostPaths::in_dir(dir))
}

pub fn diagnostic_pids(app: &AppHandle) -> Vec<(u32, &'static str)> {
    let mut pids = Vec::new();
    if let Some(pid) = app.state::<CoreState>().connection_id() {
        let role = if app.state::<CoreState>().is_tun() { "host" } else { "core" };
        pids.push((pid, role));
    }
    if let Some(pid) = pid_path(app).ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| raw.trim().parse().ok())
    {
        pids.push((pid, "core"));
    }
    if let Some(pid) = tun_paths(app).ok().and_then(|paths| paths.host_pid()) {
        pids.push((pid, "host"));
    }
    pids
}

async fn ensure_ports_free(settings: &Settings) -> Result<(), String> {
    // Claiming the port must be verified *before* spawning: otherwise a stale
    // listener makes the readiness check below succeed against someone else's
    // socket while our core dies with "address already in use".
    for port in [settings.listen_port, settings.listen_port + 1] {
        let free = tauri::async_runtime::spawn_blocking(move || port_is_free(port))
            .await
            .unwrap_or(false);
        if !free {
            return Err(t!("err.port_busy", port = port));
        }
    }
    Ok(())
}

async fn await_ready(app: &AppHandle, port: u16, timeout: Duration) -> Result<(), String> {
    let ready = tauri::async_runtime::spawn_blocking(move || wait_until_listening(port, timeout))
        .await
        .map_err(|err| t!("err.wait_core", err = err))?;
    if ready {
        return Ok(());
    }
    let log = app.state::<CoreState>().log_text();
    halt(app).await;
    let detail = if log.trim().is_empty() {
        t!("err.port_not_opened", port = port)
    } else {
        log
    };
    Err(t!("err.core_failed", detail = detail))
}

/// Stops any running core, regenerates the config and starts a fresh instance.
pub async fn start(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    start_inner(app, settings, true).await
}

pub async fn restart_keeping_log(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    start_inner(app, settings, false).await
}

async fn start_inner(app: &AppHandle, settings: &Settings, clear_log: bool) -> Result<(), String> {
    halt(app).await;
    reap_orphan(app);
    ensure_ports_free(settings).await?;
    write_config(app, settings)?;

    if settings.tun {
        start_tun(app, settings, clear_log).await
    } else {
        start_sidecar(app, settings, clear_log).await
    }
}

/// Applies a new config to the running TUN core without asking for elevation again.
pub async fn reload_tun(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    write_config(app, settings)?;
    let paths = tun_paths(app)?;
    tauri::async_runtime::spawn_blocking(move || tunhost::request_reload(&paths))
        .await
        .map_err(|err| t!("err.core_restart", err = err))??;
    await_ready(app, settings.listen_port, Duration::from_secs(15)).await
}

async fn start_sidecar(app: &AppHandle, settings: &Settings, clear_log: bool) -> Result<(), String> {
    let config = config_path(app)?;
    let config_arg = config
        .to_str()
        .ok_or_else(|| t!("err.config_path_chars"))?
        .to_string();

    let (mut rx, child) = app
        .shell()
        .sidecar("sing-box")
        .map_err(|err| t!("err.core_not_found", err = err))?
        .args(["run", "-c", &config_arg])
        .spawn()
        .map_err(|err| t!("err.core_spawn", err = err))?;

    let pid = child.pid();
    {
        let state = app.state::<CoreState>();
        remember_pid(app, pid);
        state.install(Running::Sidecar(child));
        if clear_log { state.clear_log(); }
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let state = handle.state::<CoreState>();
            match event {
                CommandEvent::Stdout(bytes) | CommandEvent::Stderr(bytes) => {
                    for line in String::from_utf8_lossy(&bytes).lines() {
                        state.record(&handle, line);
                    }
                }
                CommandEvent::Terminated(payload) => {
                    let code = payload.code.map(|c| c.to_string()).unwrap_or_else(|| "?".into());
                    let reason = t!("core.exited_code", code = code);
                    CoreState::unexpected_exit(
                        handle.clone(),
                        pid,
                        reason,
                    )
                    .await;
                    break;
                }
                _ => {}
            }
        }
    });

    await_ready(app, settings.listen_port, Duration::from_secs(5)).await
}

async fn start_tun(app: &AppHandle, settings: &Settings, clear_log: bool) -> Result<(), String> {
    let paths = tun_paths(app)?;
    let dir = paths
        .config
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| t!("err.config_dir_plain"))?;

    let host_pid = tauri::async_runtime::spawn_blocking(move || tunhost::launch(&dir))
        .await
        .map_err(|err| t!("err.core_spawn", err = err))??;

    {
        let state = app.state::<CoreState>();
        state.install(Running::Tun { host_pid });
        if clear_log { state.clear_log(); }
    }

    let handle = app.clone();
    std::thread::spawn(move || follow_tun_host(handle, host_pid, paths.log));

    // Bringing up the TUN adapter and its routes takes noticeably longer than a
    // plain listener, especially the first time Windows installs the driver.
    await_ready(app, settings.listen_port, Duration::from_secs(15)).await
}

fn follow_tun_host(app: AppHandle, host_pid: u32, log: PathBuf) {
    let mut offset = 0u64;
    let mut partial = String::new();
    let publish = |lines: Vec<String>| {
        let state = app.state::<CoreState>();
        for line in lines {
            state.record(&app, &line);
        }
    };
    loop {
        std::thread::sleep(Duration::from_millis(300));
        publish(read_new_lines(&log, &mut offset, &mut partial));

        let state = app.state::<CoreState>();
        if !state.holds(host_pid) {
            return;
        }
        if !tunhost::is_alive(host_pid) {
            publish(read_new_lines(&log, &mut offset, &mut partial));
            tauri::async_runtime::spawn(CoreState::unexpected_exit(
                app.clone(),
                host_pid,
                t!("core.exited"),
            ));
            return;
        }
    }
}

fn read_new_lines(path: &Path, offset: &mut u64, partial: &mut String) -> Vec<String> {
    let Ok(mut file) = fs::File::open(path) else {
        return Vec::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len < *offset {
        *offset = 0;
        partial.clear();
    }
    if len == *offset || file.seek(SeekFrom::Start(*offset)).is_err() {
        return Vec::new();
    }
    let mut chunk = Vec::new();
    if file.read_to_end(&mut chunk).is_err() {
        return Vec::new();
    }
    *offset += chunk.len() as u64;
    partial.push_str(&String::from_utf8_lossy(&chunk));

    let mut lines = Vec::new();
    while let Some(idx) = partial.find('\n') {
        let line = partial[..idx].trim_end().to_string();
        partial.drain(..=idx);
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// Blocks for up to a few seconds in TUN mode while the elevated host winds down.
pub fn stop(app: &AppHandle) {
    match app.state::<CoreState>().take() {
        Some(Running::Sidecar(child)) => { let _ = child.kill(); }
        Some(Running::Tun { .. }) => {}
        None => reap_orphan(app),
    }
    forget_pid(app);
    if let Ok(paths) = tun_paths(app) {
        tunhost::request_stop(&paths);
    }
}

pub async fn halt(app: &AppHandle) {
    let handle = app.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || stop(&handle)).await;
}

fn wait_until_listening(port: u16, timeout: Duration) -> bool {
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{CoreState, Running};

    #[test]
    fn an_old_exit_cannot_release_a_replacement_connection() {
        let state = CoreState::default();
        state.install(Running::Tun { host_pid: 100 });
        state.take();
        state.install(Running::Tun { host_pid: 200 });
        assert!(!state.release(100));
        assert_eq!(state.connection_id(), Some(200));
        assert!(state.release(200));
        assert!(!state.is_running());
        assert!(!state.release(200));
    }

    #[test]
    fn a_deliberate_stop_does_not_trigger_unexpected_exit_cleanup() {
        let state = CoreState::default();
        state.install(Running::Tun { host_pid: 100 });
        state.take();
        assert!(!state.release(100));
    }
}
