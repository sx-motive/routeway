//! Supervisor for the bundled sing-box sidecar.

use std::fs;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::settings::Settings;
use crate::singbox;

const LOG_CAPACITY: usize = 300;

#[derive(Default)]
pub struct CoreState {
    child: Mutex<Option<CommandChild>>,
    log: Mutex<Vec<String>>,
}

impl CoreState {
    pub fn is_running(&self) -> bool {
        self.child.lock().map(|c| c.is_some()).unwrap_or(false)
    }

    pub fn log_lines(&self) -> Vec<String> {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
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

/// Stops any running core, regenerates the config and starts a fresh instance.
pub async fn start(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    stop(app);

    let config = write_config(app, settings)?;
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

    {
        let state = app.state::<CoreState>();
        if let Ok(mut slot) = state.child.lock() {
            *slot = Some(child);
        }
        if let Ok(mut log) = state.log.lock() {
            log.clear();
        }
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
                    if let Ok(mut slot) = state.child.lock() {
                        *slot = None;
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

    let port = settings.listen_port;
    let ready = tauri::async_runtime::spawn_blocking(move || wait_until_listening(port))
        .await
        .map_err(|err| format!("не удалось дождаться запуска ядра: {err}"))?;

    if !ready {
        let log = app.state::<CoreState>().log_lines().join("\n");
        stop(app);
        let detail = if log.trim().is_empty() {
            format!("порт {port} не открылся")
        } else {
            log
        };
        return Err(format!("ядро не запустилось: {detail}"));
    }

    Ok(())
}

pub fn stop(app: &AppHandle) {
    let state = app.state::<CoreState>();
    let child = state.child.lock().ok().and_then(|mut slot| slot.take());
    if let Some(child) = child {
        let _ = child.kill();
    }
}

fn wait_until_listening(port: u16) -> bool {
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}
