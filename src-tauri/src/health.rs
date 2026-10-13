use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::settings::Settings;

const TCP_TIMEOUT: Duration = Duration::from_secs(3);
const TUNNEL_TIMEOUT: Duration = Duration::from_secs(5);
const TUNNEL_PROBE_URL: &str = "http://cp.cloudflare.com/generate_204";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Tcp,
    Tunnel,
}

#[derive(Debug, Serialize)]
pub struct Probe {
    pub mode: Mode,
    pub ms: Option<u32>,
    pub error: Option<String>,
}

impl Probe {
    fn ok(mode: Mode, started: Instant) -> Self {
        let ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
        Self {
            mode,
            ms: Some(ms.max(1)),
            error: None,
        }
    }

    fn failed(mode: Mode, error: impl ToString) -> Self {
        Self {
            mode,
            ms: None,
            error: Some(error.to_string()),
        }
    }
}

/// Without the core only reachability can be measured, so it is a TCP handshake
/// with the server port. With the core running a real request goes through the
/// tunnel, which also proves the VLESS/REALITY layer works.
pub async fn probe(settings: &Settings, through_tunnel: bool) -> Option<Probe> {
    let server = settings.server.as_ref()?;
    if through_tunnel {
        return Some(probe_tunnel(settings.listen_port + 1).await);
    }
    let address = server.address.trim().to_string();
    let port = server.port;
    let result = tauri::async_runtime::spawn_blocking(move || probe_tcp(&address, port)).await;
    Some(result.unwrap_or_else(|err| Probe::failed(Mode::Tcp, err)))
}

fn probe_tcp(address: &str, port: u16) -> Probe {
    let target = match (address, port).to_socket_addrs() {
        Ok(mut addrs) => match addrs.next() {
            Some(target) => target,
            None => return Probe::failed(Mode::Tcp, "no address"),
        },
        Err(err) => return Probe::failed(Mode::Tcp, err),
    };
    let started = Instant::now();
    match TcpStream::connect_timeout(&target, TCP_TIMEOUT) {
        Ok(_) => Probe::ok(Mode::Tcp, started),
        Err(err) => Probe::failed(Mode::Tcp, err),
    }
}

async fn probe_tunnel(probe_port: u16) -> Probe {
    let proxy = match reqwest::Proxy::all(format!("http://127.0.0.1:{probe_port}")) {
        Ok(proxy) => proxy,
        Err(err) => return Probe::failed(Mode::Tunnel, err),
    };
    let client = match reqwest::Client::builder()
        .proxy(proxy)
        .timeout(TUNNEL_TIMEOUT)
        .pool_max_idle_per_host(0)
        .build()
    {
        Ok(client) => client,
        Err(err) => return Probe::failed(Mode::Tunnel, err),
    };

    let started = Instant::now();
    match client.get(TUNNEL_PROBE_URL).send().await {
        Ok(response) if response.status().is_success() => Probe::ok(Mode::Tunnel, started),
        Ok(response) => Probe::failed(Mode::Tunnel, response.status()),
        Err(err) => Probe::failed(Mode::Tunnel, err),
    }
}
