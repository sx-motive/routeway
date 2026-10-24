use std::collections::VecDeque;
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::core::CoreState;
use crate::settings::{self, Settings};

pub const INTERVAL: Duration = Duration::from_secs(10);
pub const WINDOW: usize = 30;

const TCP_TIMEOUT: Duration = Duration::from_secs(3);
const TUNNEL_TIMEOUT: Duration = Duration::from_secs(5);
const TUNNEL_PROBE_URLS: &[&str] = &[
    "https://cp.cloudflare.com/generate_204",
    "https://www.gstatic.com/generate_204",
];

fn build_tunnel_client(probe_port: u16) -> Result<reqwest::Client, String> {
    let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{probe_port}"))
        .map_err(|err| err.to_string())?;
    reqwest::Client::builder()
        .proxy(proxy)
        .timeout(TUNNEL_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        // Each sample must test a fresh tunnel connection, including after a
        // core restart; an old keep-alive socket is not a useful readiness test.
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|err| err.to_string())
}

pub async fn verify_connection(probe_port: u16) -> Result<(), String> {
    let client = build_tunnel_client(probe_port)?;
    probe_tunnel(client)
        .await
        .map(|_| ())
        .map_err(|err| t!("err.tunnel_unavailable", err = err))
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Tcp,
    Tunnel,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    pub ms: Option<u32>,
    pub error: Option<String>,
    pub at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Health {
    pub tcp: VecDeque<Sample>,
    pub tunnel: VecDeque<Sample>,
    pub tcp_available: bool,
    pub tunnel_active: bool,
    pub recovery: crate::recovery::Phase,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            tcp: VecDeque::new(),
            tunnel: VecDeque::new(),
            tcp_available: true,
            tunnel_active: false,
            recovery: crate::recovery::Phase::Idle,
        }
    }
}

#[derive(Default)]
pub struct HealthState {
    health: Mutex<Health>,
    key: Mutex<(String, String)>,
    tunnel_client: Mutex<Option<(u16, reqwest::Client)>>,
}

impl HealthState {
    pub fn snapshot(&self) -> Health {
        self.health.lock().map(|h| h.clone()).unwrap_or_default()
    }

    fn reset_if_changed(&self, server_key: String, tunnel_key: String, active: bool, tcp_available: bool) {
        let Ok(mut current) = self.key.lock() else { return };
        if let Ok(mut health) = self.health.lock() {
            if current.0 != server_key {
                *health = Health::default();
            } else if current.1 != tunnel_key {
                health.tunnel.clear();
            }
            if health.tcp_available != tcp_available {
                health.tcp.clear();
            }
            health.tunnel_active = active;
            health.tcp_available = tcp_available;
        }
        *current = (server_key, tunnel_key);
    }

    fn push(&self, mode: Mode, sample: Sample) -> Health {
        let Ok(mut health) = self.health.lock() else {
            return Health::default();
        };
        let samples = match mode { Mode::Tcp => &mut health.tcp, Mode::Tunnel => &mut health.tunnel };
        if samples.len() >= WINDOW {
            samples.pop_front();
        }
        samples.push_back(sample);
        health.clone()
    }

    fn tunnel_client(&self, probe_port: u16) -> Result<reqwest::Client, String> {
        let mut slot = self.tunnel_client.lock().map_err(|err| err.to_string())?;
        if let Some((port, client)) = slot.as_ref() {
            if *port == probe_port {
                return Ok(client.clone());
            }
        }
        let client = build_tunnel_client(probe_port)?;
        *slot = Some((probe_port, client.clone()));
        Ok(client)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn elapsed_ms(started: Instant) -> u32 {
    (started.elapsed().as_millis().min(u32::MAX as u128) as u32).max(1)
}

fn sample(result: Result<u32, String>) -> Sample {
    match result {
        Ok(ms) => Sample {
            ms: Some(ms),
            error: None,
            at: now_ms(),
        },
        Err(error) => Sample {
            ms: None,
            error: Some(error),
            at: now_ms(),
        },
    }
}

async fn probe_tcp(address: &str, port: u16) -> Result<u32, String> {
    // Bound resolution and connection together; DNS must not stall monitoring.
    tokio::time::timeout(TCP_TIMEOUT, async {
        let targets = tokio::net::lookup_host((address, port)).await.map_err(|err| err.to_string())?;
        let mut error = "no address".to_string();
        for target in targets {
            let started = Instant::now();
            match tokio::time::timeout(Duration::from_secs(1), tokio::net::TcpStream::connect(target)).await {
                Ok(Ok(_)) => return Ok(elapsed_ms(started)),
                Ok(Err(err)) => error = err.to_string(),
                Err(_) => error = "TCP connection timed out".into(),
            }
        }
        Err(error)
    }).await.unwrap_or_else(|_| Err("TCP connection timed out".into()))
}

async fn probe_tunnel(client: reqwest::Client) -> Result<u32, String> {
    probe_urls(&client, TUNNEL_PROBE_URLS).await
}

pub async fn direct_network_available() -> bool {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(TUNNEL_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .pool_max_idle_per_host(0)
        .build();
    match client {
        Ok(client) => probe_urls(&client, TUNNEL_PROBE_URLS).await.is_ok(),
        Err(_) => false,
    }
}

pub async fn local_listener_available(port: u16) -> bool {
    tauri::async_runtime::spawn_blocking(move || {
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        TcpStream::connect_timeout(&address, Duration::from_millis(300)).is_ok()
    }).await.unwrap_or(false)
}

pub fn publish(app: &AppHandle) {
    let state = app.state::<HealthState>();
    if let Ok(mut health) = state.health.lock() {
        health.recovery = app.state::<crate::recovery::RecoveryState>().phase();
        health.tunnel_active = app.state::<CoreState>().is_running()
            || app.state::<crate::recovery::RecoveryState>().token().is_some();
        let _ = app.emit("health", health.clone());
    };
}

async fn probe_urls(client: &reqwest::Client, urls: &[&str]) -> Result<u32, String> {
    let mut errors = Vec::new();
    for url in urls {
        let started = Instant::now();
        match client.get(*url).send().await {
            Ok(response) if response.status() == reqwest::StatusCode::NO_CONTENT => {
                return Ok(elapsed_ms(started));
            }
            Ok(response) => errors.push(format!("{url}: {}", response.status())),
            Err(err) => {
                // reqwest's outer Display often only says "error sending request".
                // Keep the underlying timeout, TLS or connection error visible.
                let mut detail = err.to_string();
                let mut source = std::error::Error::source(&err);
                while let Some(error) = source {
                    detail.push_str(&format!(": {error}"));
                    source = error.source();
                }
                errors.push(detail);
            }
        }
    }
    Err(errors.join("; "))
}

/// TCP and HTTPS are separate histories. Only HTTPS drives recovery decisions.
async fn probe_once(app: &AppHandle, settings: &Settings) -> Option<Health> {
    let state = app.state::<HealthState>();
    let connection = app.state::<CoreState>().connection_id();
    let recovery = app.state::<crate::recovery::RecoveryState>();
    let token = recovery.token();
    let running = connection.is_some() || token.is_some();
    let server = settings.server.as_ref();
    let tcp_available = !(settings.tun && running);
    state.reset_if_changed(
        server
            .map(|s| format!("{}:{}", s.address.trim(), s.port))
            .unwrap_or_default(),
        format!("{connection:?}|{token:?}|{}", settings.listen_port),
        running,
        tcp_available,
    );
    let server = server?;

    // Start TCP independently so it runs alongside the existing HTTPS probe.
    let tcp_task = tcp_available.then(|| {
        let address = server.address.trim().to_string();
        let port = server.port;
        tauri::async_runtime::spawn(async move { probe_tcp(&address, port).await })
    });
    let result = if running {
        let result = match state.tunnel_client(settings.listen_port + 1) {
            Ok(client) => probe_tunnel(client).await,
            Err(err) => Err(err),
        };
        Some(result)
    } else {
        None
    };
    let tcp_result = match tcp_task {
        Some(task) => Some(task.await.unwrap_or_else(|err| Err(err.to_string()))),
        None => None,
    };
    // Discard a result from a connection that ended while the request was in flight.
    if app.state::<CoreState>().connection_id() != connection || recovery.token() != token {
        return None;
    }
    if let Some(result) = tcp_result { state.push(Mode::Tcp, sample(result)); }
    let success = result.as_ref().is_some_and(|result| result.is_ok());
    if let Some(result) = result { state.push(Mode::Tunnel, sample(result)); }
    if let Some(token) = token {
        let previous = recovery.phase();
        let diagnose = recovery.observe(token, success, connection.is_none());
        let phase = recovery.phase();
        if previous != phase && recovery.valid(token) {
            let message = match phase {
                crate::recovery::Phase::Healthy => Some(t!("recovery.restored")),
                crate::recovery::Phase::Checking => Some(t!("recovery.checking")),
                crate::recovery::Phase::Unavailable => Some(t!("recovery.unavailable")),
                _ => None,
            };
            if let Some(message) = message {
                app.state::<CoreState>().recovery_log(app, message);
            }
        }
        publish(app);
        if diagnose {
            crate::recovery::recover(app, settings, token).await;
        }
    }
    if recovery.token() != token { return None; }
    Some(state.snapshot())
}

pub fn spawn_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let settings = settings::load(&app);
            let health = probe_once(&app, &settings)
                .await
                .unwrap_or_else(|| app.state::<HealthState>().snapshot());
            let _ = app.emit("health", health);
            let recovery = app.state::<crate::recovery::RecoveryState>();
            let interval = if recovery.token().is_some() { recovery.interval() } else { INTERVAL };
            let _ = tokio::time::timeout(interval, recovery.wake.notified()).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::{build_tunnel_client, probe_urls, probe_tcp, HealthState, Mode, sample, WINDOW};

    #[test]
    fn reconnect_preserves_tcp_history_and_resets_tunnel_history() {
        let state = HealthState::default();
        state.reset_if_changed("server:443".into(), "session1".into(), true, true);
        state.push(Mode::Tcp, sample(Ok(80)));
        state.push(Mode::Tunnel, sample(Ok(260)));
        state.reset_if_changed("server:443".into(), "session2".into(), true, true);
        let health = state.snapshot();
        assert_eq!(health.tcp[0].ms, Some(80));
        assert!(health.tunnel.is_empty());
        state.reset_if_changed("other:443".into(), "session3".into(), true, true);
        assert!(state.snapshot().tcp.is_empty());
    }

    #[test]
    fn metric_windows_are_bounded_independently() {
        let state = HealthState::default();
        state.push(Mode::Tunnel, sample(Err("timeout".into())));
        for _ in 0..WINDOW + 1 { state.push(Mode::Tcp, sample(Ok(80))); }
        let health = state.snapshot();
        assert_eq!(health.tcp.len(), WINDOW);
        assert_eq!(health.tunnel.len(), 1);
        assert_eq!(health.tunnel[0].ms, None);
    }

    #[test]
    fn tun_mode_does_not_reuse_a_direct_tcp_measurement() {
        let state = HealthState::default();
        state.reset_if_changed("server:443".into(), "session1".into(), false, true);
        state.push(Mode::Tcp, sample(Ok(80)));
        state.reset_if_changed("server:443".into(), "session2".into(), true, false);
        assert!(state.snapshot().tcp.is_empty());
        assert!(!state.snapshot().tcp_available);
    }

    #[test]
    fn tcp_probe_connects_without_an_http_request() {
        tauri::async_runtime::block_on(async {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let ms = probe_tcp("127.0.0.1", port).await.unwrap();
            assert!(ms >= 1);
        });
    }

    fn endpoint(status: &str) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/generate_204", listener.local_addr().unwrap());
        let response = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let task = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            listener.set_nonblocking(true).unwrap();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "probe never arrived");
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(err) => panic!("{err}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
            stream.read(&mut [0; 4096]).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        });
        (url, task)
    }

    #[test]
    fn readiness_uses_the_explicit_local_proxy() {
        tauri::async_runtime::block_on(async {
            let (proxy_url, task) = endpoint("204 No Content");
            let port = reqwest::Url::parse(&proxy_url).unwrap().port().unwrap();
            let client = build_tunnel_client(port).unwrap();
            // .invalid cannot resolve publicly: success must come from the
            // explicit proxy, never a direct request or the OS proxy settings.
            assert!(probe_urls(&client, &["http://routeway.invalid/generate_204"])
                .await
                .is_ok());
            task.join().unwrap();
        });
    }

    #[test]
    fn another_probe_site_can_confirm_a_working_tunnel() {
        tauri::async_runtime::block_on(async {
            let (failed, first) = endpoint("503 Service Unavailable");
            let (working, second) = endpoint("204 No Content");
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            assert!(probe_urls(&client, &[&failed, &working]).await.is_ok());
            first.join().unwrap();
            second.join().unwrap();
        });
    }

    #[test]
    fn a_captive_portal_page_is_not_a_successful_probe() {
        tauri::async_runtime::block_on(async {
            let (portal, task) = endpoint("200 OK");
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let error = probe_urls(&client, &[&portal]).await.unwrap_err();
            assert!(error.contains("200 OK"));
            task.join().unwrap();
        });
    }

    #[test]
    fn failure_reports_both_probe_errors() {
        tauri::async_runtime::block_on(async {
            let (first_url, first) = endpoint("502 Bad Gateway");
            let (second_url, second) = endpoint("503 Service Unavailable");
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let error = probe_urls(&client, &[&first_url, &second_url]).await.unwrap_err();
            assert!(error.contains("502 Bad Gateway"));
            assert!(error.contains("503 Service Unavailable"));
            first.join().unwrap();
            second.join().unwrap();
        });
    }
}
