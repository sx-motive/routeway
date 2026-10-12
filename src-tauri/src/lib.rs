mod core;
mod settings;
mod singbox;
mod sysproxy;

use std::time::Duration;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_autostart::ManagerExt;

use crate::core::CoreState;
use crate::settings::Settings;

#[derive(Serialize)]
pub struct Status {
    settings: Settings,
    running: bool,
    system_proxy_active: bool,
    log: Vec<String>,
}

async fn status_of(app: &AppHandle, settings: Settings) -> Status {
    let port = settings.listen_port;
    let active = tauri::async_runtime::spawn_blocking(move || sysproxy::is_active(port))
        .await
        .unwrap_or(false);
    Status {
        settings,
        running: app.state::<CoreState>().is_running(),
        system_proxy_active: active,
        log: app.state::<CoreState>().log_lines(),
    }
}

#[tauri::command]
async fn get_status(app: AppHandle) -> Status {
    let settings = settings::load(&app);
    status_of(&app, settings).await
}

/// Persists settings. If the core is already running it is restarted so the new
/// domain list takes effect immediately.
#[tauri::command]
async fn save_settings(app: AppHandle, settings: Settings) -> Result<Status, String> {
    settings::save(&app, &settings)?;

    let autostart = app.autolaunch();
    let _ = if settings.autostart {
        autostart.enable()
    } else {
        autostart.disable()
    };

    if app.state::<CoreState>().is_running() {
        core::start(&app, &settings).await?;
    }
    Ok(status_of(&app, settings).await)
}

#[tauri::command]
async fn connect(app: AppHandle) -> Result<Status, String> {
    let settings = settings::load(&app);
    core::start(&app, &settings).await?;

    if settings.system_proxy {
        let port = settings.listen_port;
        let result = tauri::async_runtime::spawn_blocking(move || sysproxy::enable(port))
            .await
            .map_err(|err| format!("не удалось применить системный прокси: {err}"))?;
        if let Err(err) = result {
            // Routing without a system proxy is useless; roll the core back so the
            // UI never claims to be connected when nothing is routed.
            core::stop(&app);
            return Err(err);
        }
    }

    Ok(status_of(&app, settings).await)
}

#[tauri::command]
async fn disconnect(app: AppHandle) -> Result<Status, String> {
    let settings = settings::load(&app);
    teardown(&app, &settings).await;
    Ok(status_of(&app, settings).await)
}

#[tauri::command]
async fn check_ip(app: AppHandle, through_server: bool) -> Result<String, String> {
    let settings = settings::load(&app);
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(20));

    if through_server {
        // The probe inbound always exits through the server, regardless of the
        // domain list — so this answers "is my server alive" unambiguously.
        let proxy = format!("http://127.0.0.1:{}", settings.listen_port + 1);
        builder = builder.proxy(
            reqwest::Proxy::all(&proxy).map_err(|err| format!("неверный адрес прокси: {err}"))?,
        );
    } else {
        builder = builder.no_proxy();
    }

    let client = builder
        .build()
        .map_err(|err| format!("не удалось создать HTTP-клиент: {err}"))?;
    let response = client
        .get("https://api.ipify.org")
        .send()
        .await
        .map_err(|err| format!("запрос не удался: {err}"))?;
    let body = response
        .text()
        .await
        .map_err(|err| format!("не удалось прочитать ответ: {err}"))?;
    Ok(body.trim().to_string())
}

#[tauri::command]
async fn reveal_config(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = core::config_path(&app)?;
    let dir = path
        .parent()
        .ok_or_else(|| "не удалось определить каталог конфига".to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|err| format!("не удалось открыть каталог: {err}"))
}

/// Stops the core and hands the network back to the system. Always safe to call.
async fn teardown(app: &AppHandle, settings: &Settings) {
    core::stop(app);
    let port = settings.listen_port;
    let _ = tauri::async_runtime::spawn_blocking(move || {
        if sysproxy::is_active(port) {
            let _ = sysproxy::disable();
        }
    })
    .await;
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Открыть Routeway", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выйти", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().unwrap())
        .tooltip("Routeway")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let settings = settings::load(&handle);
                    teardown(&handle, &settings).await;
                    handle.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(CoreState::default())
        .invoke_handler(tauri::generate_handler![
            get_status,
            save_settings,
            connect,
            disconnect,
            check_ip,
            reveal_config
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window must not kill routing — Routeway lives in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("не удалось запустить Routeway");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Last chance to restore the system proxy; the async runtime is still
            // alive here, so block on the teardown.
            let handle = handle.clone();
            tauri::async_runtime::block_on(async move {
                let settings = settings::load(&handle);
                teardown(&handle, &settings).await;
            });
        }
    });
}
