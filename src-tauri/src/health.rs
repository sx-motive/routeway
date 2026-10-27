use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::core::CoreState;
use crate::recovery::RecoveryState;
use crate::settings;

pub const INTERVAL: Duration = Duration::from_secs(60);
pub const WINDOW: usize = 30;

const TUNNEL_TIMEOUT: Duration = Duration::from_secs(5);
const TUNNEL_PROBE_URL: &str = "https://cp.cloudflare.com/generate_204";

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
    probe(&client, TUNNEL_PROBE_URL)
        .await
        .map(|_| ())
        .map_err(|err| t!("err.tunnel_unavailable", err = err))
}

#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    pub ms: Option<u32>,
    pub error: Option<String>,
    pub at: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Health {
    pub tunnel: VecDeque<Sample>,
    pub tunnel_active: bool,
}

#[derive(Default)]
pub struct HealthState {
    health: Mutex<Health>,
    session: Mutex<Option<(u64, u16)>>,
}

impl HealthState {
    pub fn snapshot(&self) -> Health {
        self.health.lock().map(|h| h.clone()).unwrap_or_default()
    }

    fn reset_if_changed(&self, session: Option<(u64, u16)>) {
        let Ok(mut current) = self.session.lock() else { return };
        if *current != session {
            if let Ok(mut health) = self.health.lock() {
                health.tunnel.clear();
            }
            *current = session;
        }
    }

    fn push(&self, sample: Sample) {
        let Ok(mut health) = self.health.lock() else { return };
        if health.tunnel.len() >= WINDOW {
            health.tunnel.pop_front();
        }
        health.tunnel.push_back(sample);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn sample(result: Result<u32, String>) -> Sample {
    let (ms, error) = match result {
        Ok(ms) => (Some(ms), None),
        Err(error) => (None, Some(error)),
    };
    Sample { ms, error, at: now_ms() }
}

async fn probe(client: &reqwest::Client, url: &str) -> Result<u32, String> {
    let started = Instant::now();
    match client.get(url).send().await {
        Ok(response) if response.status() == reqwest::StatusCode::NO_CONTENT => {
            Ok((started.elapsed().as_millis().min(u32::MAX as u128) as u32).max(1))
        }
        Ok(response) => Err(format!("{url}: {}", response.status())),
        Err(err) => {
            // reqwest's outer Display often only says "error sending request".
            // Keep the underlying timeout, TLS or connection error visible.
            let mut detail = err.to_string();
            let mut source = std::error::Error::source(&err);
            while let Some(error) = source {
                detail.push_str(&format!(": {error}"));
                source = error.source();
            }
            Err(detail)
        }
    }
}

pub fn publish(app: &AppHandle) {
    let state = app.state::<HealthState>();
    if let Ok(mut health) = state.health.lock() {
        health.tunnel_active = app.state::<CoreState>().is_running()
            || app.state::<RecoveryState>().token().is_some();
        let _ = app.emit("health", health.clone());
    };
}

pub async fn probe_once(app: &AppHandle) {
    let state = app.state::<HealthState>();
    let recovery = app.state::<RecoveryState>();
    let token = recovery.token();
    let port = settings::load(app).listen_port + 1;
    state.reset_if_changed(token.map(|token| (token, port)));
    if token.is_some() {
        let result = match build_tunnel_client(port) {
            Ok(client) => probe(&client, TUNNEL_PROBE_URL).await,
            Err(err) => Err(err),
        };
        // Discard a result from a connection that ended while the request was in flight.
        if recovery.token() == token {
            state.push(sample(result));
        }
    }
    publish(app);
}

pub fn spawn_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            probe_once(&app).await;
            let wake = &app.state::<RecoveryState>().wake;
            let _ = tokio::time::timeout(INTERVAL, wake.notified()).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::{build_tunnel_client, probe, sample, HealthState, WINDOW};

    #[test]
    fn a_new_session_resets_the_history() {
        let state = HealthState::default();
        state.reset_if_changed(Some((1, 2081)));
        state.push(sample(Ok(260)));
        state.reset_if_changed(Some((1, 2081)));
        assert_eq!(state.snapshot().tunnel.len(), 1);
        state.reset_if_changed(Some((2, 2081)));
        assert!(state.snapshot().tunnel.is_empty());
    }

    #[test]
    fn history_is_bounded() {
        let state = HealthState::default();
        for _ in 0..WINDOW + 1 { state.push(sample(Ok(80))); }
        assert_eq!(state.snapshot().tunnel.len(), WINDOW);
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
            assert!(probe(&client, "http://routeway.invalid/generate_204").await.is_ok());
            task.join().unwrap();
        });
    }

    #[test]
    fn a_captive_portal_page_is_not_a_successful_probe() {
        tauri::async_runtime::block_on(async {
            let (portal, task) = endpoint("200 OK");
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let error = probe(&client, &portal).await.unwrap_err();
            assert!(error.contains("200 OK"));
            task.join().unwrap();
        });
    }
}
