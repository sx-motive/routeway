//! Supervisor for the bundled sing-box sidecar.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

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
    log: Mutex<Vec<String>>,
}

impl CoreState {
    pub fn is_running(&self) -> bool {
        self.running.lock().map(|r| r.is_some()).unwrap_or(false)
    }

    pub fn is_tun(&self) -> bool {
        self.running
            .lock()
            .map(|r| matches!(*r, Some(Running::Tun { .. })))
            .unwrap_or(false)
    }

    pub fn log_lines(&self) -> Vec<String> {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
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

    fn clear_log(&self) {
        if let Ok(mut log) = self.log.lock() {
            log.clear();
        }
    }

    fn push_log(&self, line: String) {
        if let Ok(mut log) = self.log.lock() {
            if log.len() >= LOG_CAPACITY {
                log.remove(0);
            }
            log.push(line);
        }
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
        .map_err(|err| format!("не удалось определить каталог настроек: {err}"))?;
    fs::create_dir_all(&dir).map_err(|err| format!("не удалось создать {dir:?}: {err}"))?;
    Ok(dir.join("sing-box.json"))
}

pub fn write_config(app: &AppHandle, settings: &Settings) -> Result<PathBuf, String> {
    let config = singbox::build_config(settings)?;
    let path = config_path(app)?;
    let body = serde_json::to_string_pretty(&config)
        .map_err(|err| format!("не удалось сериализовать конфиг: {err}"))?;
    fs::write(&path, body).map_err(|err| format!("не удалось записать конфиг: {err}"))?;
    Ok(path)
}

fn tun_paths(app: &AppHandle) -> Result<HostPaths, String> {
    let config = config_path(app)?;
    let dir = config
        .parent()
        .ok_or_else(|| "не удалось определить каталог настроек".to_string())?;
    Ok(HostPaths::in_dir(dir))
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
            return Err(format!(
                "порт {port} занят другим процессом — выберите другой порт в настройках"
            ));
        }
    }
    Ok(())
}

async fn await_ready(app: &AppHandle, port: u16, timeout: Duration) -> Result<(), String> {
    let ready = tauri::async_runtime::spawn_blocking(move || wait_until_listening(port, timeout))
        .await
        .map_err(|err| format!("не удалось дождаться запуска ядра: {err}"))?;
    if ready {
        return Ok(());
    }
    let log = app.state::<CoreState>().log_lines().join("\n");
    halt(app).await;
    let detail = if log.trim().is_empty() {
        format!("порт {port} не открылся")
    } else {
        log
    };
    Err(format!("ядро не запустилось: {detail}"))
}

/// Stops any running core, regenerates the config and starts a fresh instance.
pub async fn start(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    halt(app).await;
    reap_orphan(app);
    ensure_ports_free(settings).await?;
    write_config(app, settings)?;

    if settings.tun {
        start_tun(app, settings).await
    } else {
        start_sidecar(app, settings).await
    }
}

/// Applies a new config to the running TUN core without asking for elevation again.
pub async fn reload_tun(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    write_config(app, settings)?;
    let paths = tun_paths(app)?;
    tauri::async_runtime::spawn_blocking(move || tunhost::request_reload(&paths))
        .await
        .map_err(|err| format!("не удалось перезапустить ядро: {err}"))??;
    await_ready(app, settings.listen_port, Duration::from_secs(15)).await
}

async fn start_sidecar(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let config = config_path(app)?;
    let config_arg = config
        .to_str()
        .ok_or_else(|| "путь к конфигу содержит недопустимые символы".to_string())?
        .to_string();

    let (mut rx, child) = app
        .shell()
        .sidecar("sing-box")
        .map_err(|err| format!("не удалось найти ядро sing-box: {err}"))?
        .args(["run", "-c", &config_arg])
        .spawn()
        .map_err(|err| format!("не удалось запустить ядро: {err}"))?;

    let pid = child.pid();
    {
        let state = app.state::<CoreState>();
        remember_pid(app, pid);
        state.install(Running::Sidecar(child));
        state.clear_log();
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            let state = handle.state::<CoreState>();
            match event {
                CommandEvent::Stdout(bytes) | CommandEvent::Stderr(bytes) => {
                    let line = String::from_utf8_lossy(&bytes).trim_end().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    state.push_log(line.clone());
                    let _ = handle.emit("core-log", line);
                }
                CommandEvent::Terminated(payload) => {
                    if state.release(pid) {
                        forget_pid(&handle);
                    }
                    let reason = format!("ядро остановилось (код {:?})", payload.code);
                    state.push_log(reason.clone());
                    let _ = handle.emit("core-exit", reason);
                    break;
                }
                _ => {}
            }
        }
    });

    await_ready(app, settings.listen_port, Duration::from_secs(5)).await
}

async fn start_tun(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let paths = tun_paths(app)?;
    let dir = paths
        .config
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "не удалось определить каталог настроек".to_string())?;

    let host_pid = tauri::async_runtime::spawn_blocking(move || tunhost::launch(&dir))
        .await
        .map_err(|err| format!("не удалось запустить ядро: {err}"))??;

    {
        let state = app.state::<CoreState>();
        state.install(Running::Tun { host_pid });
        state.clear_log();
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
            state.push_log(line.clone());
            let _ = app.emit("core-log", line);
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
            state.release(host_pid);
            let reason = "ядро остановилось".to_string();
            state.push_log(reason.clone());
            let _ = app.emit("core-exit", reason);
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
    if let Some(Running::Sidecar(child)) = app.state::<CoreState>().take() {
        let _ = child.kill();
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
