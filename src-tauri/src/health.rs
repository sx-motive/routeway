use std::collections::VecDeque;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::core::CoreState;
use crate::settings::{self, Settings};

pub const INTERVAL: Duration = Duration::from_secs(5);
pub const WINDOW: usize = 30;

const TCP_TIMEOUT: Duration = Duration::from_secs(3);
const TUNNEL_TIMEOUT: Duration = Duration::from_secs(5);
const TUNNEL_PROBE_URL: &str = "http://cp.cloudflare.com/generate_204";

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

#[derive(Debug, Clone, Serialize, Default)]
pub struct Health {
    pub mode: Option<Mode>,
    pub samples: VecDeque<Sample>,
}

#[derive(Default)]
pub struct HealthState {
    health: Mutex<Health>,
    key: Mutex<String>,
    tunnel_client: Mutex<Option<(u16, reqwest::Client)>>,
}

impl HealthState {
    pub fn snapshot(&self) -> Health {
        self.health.lock().map(|h| h.clone()).unwrap_or_default()
    }

    fn reset_if_changed(&self, key: String) {
        let Ok(mut current) = self.key.lock() else { return };
        if *current == key {
            return;
        }
        *current = key;
        if let Ok(mut health) = self.health.lock() {
            *health = Health::default();
        }
    }

    fn push(&self, mode: Mode, sample: Sample) -> Health {
        let Ok(mut health) = self.health.lock() else {
            return Health::default();
        };
        health.mode = Some(mode);
        if health.samples.len() >= WINDOW {
            health.samples.pop_front();
        }
        health.samples.push_back(sample);
        health.clone()
    }

    fn tunnel_client(&self, probe_port: u16) -> Result<reqwest::Client, String> {
        let mut slot = self.tunnel_client.lock().map_err(|err| err.to_string())?;
        if let Some((port, client)) = slot.as_ref() {
            if *port == probe_port {
                return Ok(client.clone());
            }
        }
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{probe_port}"))
            .map_err(|err| err.to_string())?;
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(TUNNEL_TIMEOUT)
            .build()
            .map_err(|err| err.to_string())?;
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

fn probe_tcp(address: &str, port: u16) -> Result<u32, String> {
    let target = (address, port)
        .to_socket_addrs()
        .map_err(|err| err.to_string())?
        .next()
        .ok_or_else(|| "no address".to_string())?;
    let started = Instant::now();
    TcpStream::connect_timeout(&target, TCP_TIMEOUT).map_err(|err| err.to_string())?;
    Ok(elapsed_ms(started))
}

async fn probe_tunnel(client: reqwest::Client) -> Result<u32, String> {
    let started = Instant::now();
    let response = client
        .get(TUNNEL_PROBE_URL)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(response.status().to_string());
    }
    Ok(elapsed_ms(started))
}

/// Without the core only reachability can be measured, so it is a TCP handshake
/// with the server port. With the core running a real request goes through the
/// tunnel over a kept-alive connection, which also proves the VLESS layer works.
async fn probe_once(app: &AppHandle, settings: &Settings) -> Option<Health> {
    let state = app.state::<HealthState>();
    let running = app.state::<CoreState>().is_running();
    let server = settings.server.as_ref();
    state.reset_if_changed(format!(
        "{running}|{}",
        server
            .map(|s| format!("{}:{}", s.address.trim(), s.port))
            .unwrap_or_default()
    ));
    let server = server?;

    let (mode, result) = if running {
        let result = match state.tunnel_client(settings.listen_port + 1) {
            Ok(client) => probe_tunnel(client).await,
            Err(err) => Err(err),
        };
        (Mode::Tunnel, result)
    } else {
        let address = server.address.trim().to_string();
        let port = server.port;
        let result = tauri::async_runtime::spawn_blocking(move || probe_tcp(&address, port))
            .await
            .unwrap_or_else(|err| Err(err.to_string()));
        (Mode::Tcp, result)
    };
    Some(state.push(mode, sample(result)))
}

pub fn spawn_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let settings = settings::load(&app);
            let health = probe_once(&app, &settings)
                .await
                .unwrap_or_else(|| app.state::<HealthState>().snapshot());
            let _ = app.emit("health", health);
            tokio::time::sleep(INTERVAL).await;
        }
    });
}
