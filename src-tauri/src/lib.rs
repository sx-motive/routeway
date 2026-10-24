#[macro_use]
mod i18n;
mod core;
mod corelog;
mod diagnostics;
mod health;
mod recovery;
mod settings;
mod singbox;
mod startup;
mod sysproxy;
mod tunhost;

pub use tunhost::{run_host as run_tun_host, HOST_FLAG as TUN_HOST_FLAG};

use std::time::Duration;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_autostart::ManagerExt;

use crate::core::CoreState;
use crate::settings::Settings;

/// Serialize connection changes, including cleanup after an unexpected exit.
#[derive(Default)]
struct ConnectionState(tokio::sync::Mutex<()>);

#[derive(Serialize)]
pub struct Status {
    settings: Settings,
    running: bool,
    connection_requested: bool,
    system_proxy_active: bool,
    tun_active: bool,
    log: Vec<corelog::LogEvent>,
}

async fn status_of(app: &AppHandle, settings: Settings) -> Status {
    let port = settings.listen_port;
    let active = tauri::async_runtime::spawn_blocking(move || sysproxy::is_active(port))
        .await
        .unwrap_or(false);
    Status {
        settings,
        running: app.state::<CoreState>().is_running(),
        connection_requested: app.state::<recovery::RecoveryState>().token().is_some(),
        system_proxy_active: active,
        tun_active: app.state::<CoreState>().is_tun(),
        log: app.state::<CoreState>().log_events(),
    }
}

#[tauri::command]
async fn get_status(app: AppHandle) -> Status {
    let settings = settings::load(&app);
    status_of(&app, settings).await
}

#[derive(Serialize)]
struct StartupStatus {
    status: Status,
    error: Option<String>,
}

/// Called when the webview is ready. Waiting here keeps the UI from offering a
/// second connection while startup is still starting the core or awaiting UAC.
#[tauri::command]
async fn initialize(app: AppHandle) -> StartupStatus {
    let settings = settings::load(&app);
    let error = app
        .state::<startup::Startup>()
        .run(&settings, async { connect(app.clone()).await.map(|_| ()) })
        .await
        .err();
    StartupStatus {
        status: get_status(app).await,
        error,
    }
}

/// Persists settings. If the core is already running it is restarted so the new
/// domain list takes effect immediately.
#[tauri::command]
async fn save_settings(app: AppHandle, settings: Settings) -> Result<Status, String> {
    let state = app.state::<ConnectionState>();
    let _guard = state.0.lock().await;
    apply(&app, settings).await
}

#[tauri::command]
fn get_rules(app: AppHandle) -> String {
    settings::rules_text(&app)
}

#[tauri::command]
fn reveal_rules(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = settings::ensure_rules_file(&app)?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|err| t!("err.open_dir", err = err))
}

#[tauri::command]
async fn save_rules(app: AppHandle, text: String) -> Result<Status, String> {
    let state = app.state::<ConnectionState>();
    let _guard = state.0.lock().await;
    let rules = settings::Rules::parse(&text)?;
    let mut settings = settings::load(&app);
    settings.processes = rules.apps;
    settings.domains = rules.domains;
    settings.direct = rules.direct;
    apply(&app, settings).await
}

async fn apply(app: &AppHandle, settings: Settings) -> Result<Status, String> {
    let app = app.clone();
    let previous = settings::load(&app);
    let config_changed = singbox::build_config(&previous).ok()
        != singbox::build_config(&settings).ok();
    settings::save(&app, &settings)?;
    if i18n::current() != i18n::parse(&settings.language) {
        i18n::set(&settings.language);
        relabel_tray(&app);
    }

    let autostart = app.autolaunch();
    let _ = if settings.autostart {
        autostart.enable()
    } else {
        autostart.disable()
    };

    let state = app.state::<CoreState>();
    if state.is_running() {
        let recovery_token = if config_changed || previous.system_proxy != settings.system_proxy {
            let was_requested = app.state::<recovery::RecoveryState>().token().is_some();
            let token = recovery::cancel(&app);
            was_requested.then_some(token)
        } else { None };
        // Restore the previous port before replacing its listener. Any failed
        // restart must leave the OS with a usable proxy configuration.
        if config_changed {
            release_system_proxy(&app, previous.listen_port).await;
        }
        let result = async {
            if config_changed {
                if state.is_tun() && settings.tun {
                    core::reload_tun(&app, &settings).await?;
                } else {
                    core::start(&app, &settings).await?;
                }
            }
            if config_changed || (!previous.system_proxy && settings.system_proxy && !settings.tun) {
                health::verify_connection(settings.listen_port + 1).await?;
            }
            reconcile_system_proxy(&app, &settings).await
        }
        .await;
        if let Err(err) = result {
            recovery::cancel(&app);
            teardown(&app, &settings).await;
            return Err(err);
        }
        if let Some(token) = recovery_token {
            recovery::activate(&app, token);
        }
    }
    Ok(status_of(&app, settings).await)
}

/// Brings the system proxy in line with the settings while the core is running.
async fn reconcile_system_proxy(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let port = settings.listen_port;

    if settings.system_proxy && !settings.tun {
        // A changed port or direct list reads as "not current" and gets rewritten.
        let direct = settings.normalized_direct();
        let probe = direct.clone();
        let current = tauri::async_runtime::spawn_blocking(move || sysproxy::is_current(port, &probe))
            .await
            .unwrap_or(false);
        if !current {
            snapshot_system_proxy(app, port).await;
            return tauri::async_runtime::spawn_blocking(move || sysproxy::enable(port, &direct))
                .await
                .map_err(|err| t!("err.apply_proxy", err = err))?;
        }
        return Ok(());
    }

    release_system_proxy(app, port).await;
    Ok(())
}

/// Remembers the system proxy configuration that existed before Routeway took
/// over, so turning routing off can put it back instead of clearing it.
async fn snapshot_system_proxy(app: &AppHandle, port: u16) {
    // Never snapshot our own settings: that would make the backup a no-op and
    // lose whatever the user actually had configured.
    if settings::has_proxy_backup(app) {
        return;
    }
    let already_ours = tauri::async_runtime::spawn_blocking(move || sysproxy::is_active(port))
        .await
        .unwrap_or(false);
    if already_ours {
        return;
    }
    if let Ok(Some(raw)) = tauri::async_runtime::spawn_blocking(sysproxy::capture).await {
        let _ = settings::save_proxy_backup(app, &raw);
    }
}

/// Hands the system proxy back: restores the pre-Routeway configuration when we
/// have one, otherwise just clears what we set.
async fn release_system_proxy(app: &AppHandle, port: u16) {
    let backup = settings::read_proxy_backup(app);
    let result = tauri::async_runtime::spawn_blocking(move || match backup {
        Some(raw) => sysproxy::restore(&raw),
        None if sysproxy::is_active(port) => sysproxy::disable(),
        None => Ok(()),
    })
    .await;
    // Retain the snapshot if restoration failed so a later cleanup can retry.
    if matches!(result, Ok(Ok(()))) {
        settings::clear_proxy_backup(app);
    }
}

#[tauri::command]
async fn set_language(app: AppHandle, language: String) -> Result<Status, String> {
    let state = app.state::<ConnectionState>();
    let _guard = state.0.lock().await;
    let mut settings = settings::load(&app);
    settings.language = language;
    settings::save(&app, &settings)?;
    i18n::set(&settings.language);
    relabel_tray(&app);
    Ok(status_of(&app, settings).await)
}

#[tauri::command]
async fn connect(app: AppHandle) -> Result<Status, String> {
    let token = recovery::cancel(&app);
    let state = app.state::<ConnectionState>();
    let _guard = state.0.lock().await;
    if !app.state::<recovery::RecoveryState>().current(token) {
        return Err(t!("err.cancelled"));
    }
    let settings = settings::load(&app);
    release_system_proxy(&app, settings.listen_port).await;
    let result = async {
        core::start(&app, &settings).await?;
        // An open local port does not prove DNS, TLS or VLESS is working.
        // Verify the complete path before sending other applications through it.
        health::verify_connection(settings.listen_port + 1).await?;
        if !app.state::<recovery::RecoveryState>().current(token) {
            return Err(t!("err.cancelled"));
        }
        reconcile_system_proxy(&app, &settings).await
    }
    .await;
    if let Err(err) = result {
        teardown(&app, &settings).await;
        return Err(err);
    }

    if !recovery::activate(&app, token) {
        teardown(&app, &settings).await;
        return Err(t!("err.cancelled"));
    }

    Ok(status_of(&app, settings).await)
}

#[tauri::command]
async fn disconnect(app: AppHandle) -> Result<Status, String> {
    recovery::cancel(&app);
    let state = app.state::<ConnectionState>();
    let _guard = state.0.lock().await;
    recovery::cancel(&app);
    let settings = settings::load(&app);
    teardown(&app, &settings).await;
    Ok(status_of(&app, settings).await)
}

#[tauri::command]
async fn get_diagnostics(app: AppHandle) -> Result<diagnostics::Diagnostics, String> {
    tauri::async_runtime::spawn_blocking(move || diagnostics::snapshot(&app))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn restart_core(app: AppHandle) -> Result<Status, String> {
    connect(app).await
}

#[tauri::command]
async fn clean_core(app: AppHandle) -> Result<Status, String> {
    recovery::cancel(&app);
    let state = app.state::<ConnectionState>();
    let _guard = state.0.lock().await;
    recovery::cancel(&app);
    let settings = settings::load(&app);
    teardown(&app, &settings).await;
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || core::reap_orphan(&handle))
        .await
        .map_err(|err| err.to_string())?;
    Ok(status_of(&app, settings).await)
}

#[tauri::command]
async fn restart_app(app: AppHandle) -> Result<(), String> {
    clean_core(app.clone()).await?;
    app.request_restart();
    Ok(())
}

#[tauri::command]
fn get_health(app: AppHandle) -> health::Health {
    app.state::<health::HealthState>().snapshot()
}

#[tauri::command]
fn minimize_window(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|err| err.to_string())
}

#[tauri::command]
fn hide_window(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(|err| err.to_string())
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
            reqwest::Proxy::all(&proxy).map_err(|err| t!("err.bad_proxy", err = err))?,
        );
    } else {
        builder = builder.no_proxy();
    }

    let client = builder
        .build()
        .map_err(|err| t!("err.http_client", err = err))?;
    let response = client
        .get("https://api.ipify.org")
        .send()
        .await
        .map_err(|err| t!("err.request", err = err))?;
    let body = response
        .text()
        .await
        .map_err(|err| t!("err.read_response", err = err))?;
    Ok(body.trim().to_string())
}

#[tauri::command]
async fn reveal_config(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = core::config_path(&app)?;
    let dir = path
        .parent()
        .ok_or_else(|| t!("err.config_dir_plain"))?;
    app.opener()
        .open_path(dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|err| t!("err.open_dir", err = err))
}

/// Stops the core and hands the network back to the system. Always safe to call.
async fn teardown(app: &AppHandle, settings: &Settings) {
    release_system_proxy(app, settings.listen_port).await;
    core::halt(app).await;
}

struct TrayItems {
    show: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

fn relabel_tray(app: &AppHandle) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.show.set_text(t!("tray.show"));
        let _ = items.quit.set_text(t!("tray.quit"));
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", t!("tray.show"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", t!("tray.quit"), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    app.manage(TrayItems {
        show: show.clone(),
        quit: quit.clone(),
    });

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
                    let _ = disconnect(handle.clone()).await;
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
        // Must run before other plugins and setup: a duplicate must not reap
        // the active instance's core or restore its system proxy settings.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(CoreState::default())
        .manage(ConnectionState::default())
        .manage(startup::Startup::default())
        .manage(health::HealthState::default())
        .manage(recovery::RecoveryState::default())
        .invoke_handler(tauri::generate_handler![
            initialize,
            get_status,
            save_settings,
            set_language,
            get_rules,
            save_rules,
            reveal_rules,
            connect,
            disconnect,
            check_ip,
            get_health,
            get_diagnostics,
            restart_core,
            clean_core,
            restart_app,
            minimize_window,
            hide_window,
            reveal_config
        ])
        .setup(|app| {
            // A previous run that was killed rather than quit leaves its core
            // holding the port; clear it before anything else needs that port.
            i18n::set(&settings::load(app.handle()).language);
            core::reap_orphan(app.handle());
            // A run that was killed in proxy mode leaves the OS pointing at a port
            // nobody listens on, which cuts off every app that honours the proxy.
            let port = settings::load(app.handle()).listen_port;
            tauri::async_runtime::block_on(release_system_proxy(app.handle(), port));
            health::spawn_monitor(app.handle().clone());
            build_tray(app.handle())?;
            // A tray app can end up launched without its window surfacing (the
            // process starts in the background and the window never activates).
            // Showing it explicitly makes the first launch deterministic.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
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
        .expect("failed to start Routeway");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            recovery::cancel(handle);
            // Last chance to restore the system proxy; the async runtime is still
            // alive here, so block on the teardown.
            let handle = handle.clone();
            tauri::async_runtime::block_on(async move {
                let connection = handle.state::<ConnectionState>();
                let _guard = connection.0.lock().await;
                let settings = settings::load(&handle);
                teardown(&handle, &settings).await;
            });
        }
    });
}
