use std::collections::{HashMap, VecDeque};

use serde::Serialize;

const MAX_TRACKED_CONNECTIONS: usize = 4096;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Started,
    Stopped,
    Listening,
    Interface,
    ConnectionFailed,
    Other,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Route {
    Direct,
    Proxy,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LogEvent {
    pub time: String,
    pub level: Level,
    pub kind: Kind,
    pub app: Option<String>,
    pub target: Option<String>,
    pub domain: Option<String>,
    pub route: Option<Route>,
    pub reason: Option<String>,
    pub detail: Option<String>,
    pub raw: String,
}

impl LogEvent {
    pub fn stopped(message: String) -> Self {
        Self {
            time: now(),
            level: Level::Warn,
            kind: Kind::Stopped,
            app: None,
            target: None,
            domain: None,
            route: None,
            reason: None,
            detail: None,
            raw: message,
        }
    }

    fn new(time: String, level: Level, kind: Kind, raw: &str) -> Self {
        Self {
            time,
            level,
            kind,
            app: None,
            target: None,
            domain: None,
            route: None,
            reason: None,
            detail: None,
            raw: raw.to_string(),
        }
    }
}

fn now() -> String {
    String::new()
}

#[derive(Default)]
struct Connection {
    app: Option<String>,
    target: Option<String>,
    domain: Option<String>,
    route: Option<Route>,
}

/// Turns sing-box output into events a person can read. Successful connections
/// are only remembered as context, so a failure can say which app tried to
/// reach which host, and how.
#[derive(Default)]
pub struct LogParser {
    connections: HashMap<u64, Connection>,
    order: VecDeque<u64>,
}

struct Parsed<'a> {
    time: String,
    level: Level,
    id: Option<u64>,
    module: &'a str,
    message: &'a str,
}

fn parse_line(line: &str) -> Option<Parsed<'_>> {
    let mut rest = line.trim();
    let mut parts = rest.splitn(5, ' ');
    let zone = parts.next()?;
    if !zone.starts_with('+') && !zone.starts_with('-') {
        return None;
    }
    let _date = parts.next()?;
    let time = parts.next()?.to_string();
    let level = match parts.next()? {
        "ERROR" | "FATAL" | "PANIC" => Level::Error,
        "WARN" => Level::Warn,
        _ => Level::Info,
    };
    rest = parts.next().unwrap_or("");

    let mut id = None;
    if let Some(body) = rest.strip_prefix('[') {
        if let Some((tag, after)) = body.split_once("] ") {
            id = tag.split_whitespace().next().and_then(|v| v.parse().ok());
            rest = after;
        }
    }

    let (module, message) = rest.split_once(": ").unwrap_or(("", rest));
    Some(Parsed {
        time,
        level,
        id,
        module,
        message,
    })
}

fn route_of(module: &str) -> Option<Route> {
    if module.starts_with("outbound/direct") {
        Some(Route::Direct)
    } else if module.starts_with("outbound/") {
        Some(Route::Proxy)
    } else {
        None
    }
}

fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

fn host_of(target: &str) -> &str {
    target
        .rsplit_once(':')
        .map(|(host, _)| host)
        .unwrap_or(target)
        .trim_matches(['[', ']'])
}

fn is_ip(host: &str) -> bool {
    host.parse::<std::net::IpAddr>().is_ok()
}

impl LogParser {
    pub fn ingest(&mut self, line: &str) -> Option<LogEvent> {
        let line = line.trim_end();
        if line.is_empty() {
            return None;
        }
        let Some(parsed) = parse_line(line) else {
            return Some(LogEvent::new(now(), Level::Info, Kind::Other, line));
        };

        if let Some(id) = parsed.id {
            return self.connection_line(id, &parsed, line);
        }

        let Parsed {
            time,
            level,
            module,
            message,
            ..
        } = parsed;

        if let Some(after) = line.split("sing-box started (").nth(1) {
            let mut event = LogEvent::new(time, Level::Info, Kind::Started, line);
            event.detail = after.split(')').next().map(str::to_string);
            return Some(event);
        }
        if let Some(addr) = message.strip_prefix("tcp server started at ") {
            let mut event = LogEvent::new(time, Level::Info, Kind::Listening, line);
            event.target = Some(addr.trim().to_string());
            return Some(event);
        }
        if module == "network" {
            if let Some(name) = message.strip_prefix("updated default interface ") {
                let mut event = LogEvent::new(time, Level::Info, Kind::Interface, line);
                event.detail = name.split(',').next().map(|n| n.trim().to_string());
                return Some(event);
            }
            return None;
        }
        if level == Level::Info {
            return None;
        }
        let mut event = LogEvent::new(time, level, Kind::Other, line);
        event.detail = Some(message.to_string());
        Some(event)
    }

    fn remember(&mut self, id: u64) -> &mut Connection {
        if !self.connections.contains_key(&id) {
            if self.order.len() >= MAX_TRACKED_CONNECTIONS {
                if let Some(oldest) = self.order.pop_front() {
                    self.connections.remove(&oldest);
                }
            }
            self.order.push_back(id);
        }
        self.connections.entry(id).or_default()
    }

    fn connection_line(&mut self, id: u64, parsed: &Parsed, line: &str) -> Option<LogEvent> {
        let message = parsed.message;

        if parsed.level == Level::Info {
            let conn = self.remember(id);
            if let Some(path) = message.strip_prefix("found process path: ") {
                conn.app = Some(file_name(path.trim()));
            } else if let Some(target) = message.strip_prefix("inbound connection to ") {
                conn.target = Some(target.trim().to_string());
                if !is_ip(host_of(target.trim())) {
                    conn.domain = Some(host_of(target.trim()).to_string());
                }
            } else if let Some(sniffed) = message.split("domain: ").nth(1) {
                let domain = sniffed.split([',', ' ']).next().unwrap_or("").trim();
                if !domain.is_empty() {
                    conn.domain = Some(domain.to_string());
                }
            } else if message.starts_with("outbound connection to ")
                || message.starts_with("outbound packet connection")
            {
                conn.route = route_of(parsed.module).or(conn.route);
            }
            return None;
        }

        let conn = self.connections.remove(&id).unwrap_or_default();
        let mut event = LogEvent::new(parsed.time.clone(), parsed.level, Kind::Other, line);
        event.app = conn.app;
        event.domain = conn.domain;
        event.route = conn.route;
        event.target = conn.target;

        if let Some(rest) = message.strip_prefix("open connection to ") {
            event.kind = Kind::ConnectionFailed;
            if let Some((target, tail)) = rest.split_once(" using ") {
                event.target = Some(target.trim().to_string());
                let outbound = tail.split(':').next().unwrap_or("");
                event.route = route_of(outbound).or(event.route);
            }
        }
        if event.domain.is_none() {
            if let Some(target) = &event.target {
                let host = host_of(target);
                if !is_ip(host) {
                    event.domain = Some(host.to_string());
                }
            }
        }
        event.reason = message.rsplit(": ").next().map(|r| r.trim().to_string());
        if event.kind == Kind::Other {
            event.detail = Some(format!("{}: {}", parsed.module, message));
        }
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(parser: &mut LogParser, lines: &[&str]) -> Vec<LogEvent> {
        lines.iter().filter_map(|l| parser.ingest(l)).collect()
    }

    #[test]
    fn startup_lines_become_lifecycle_events() {
        let mut parser = LogParser::default();
        let events = feed(
            &mut parser,
            &[
                "+0300 2026-10-13 16:42:48 INFO inbound/mixed[in]: tcp server started at 127.0.0.1:33080",
                "+0300 2026-10-13 16:42:48 INFO network: updated default interface Ethernet, index 5",
                "+0300 2026-10-13 16:42:48 INFO sing-box started (0.08s)",
                "+0300 2026-10-13 16:42:48 INFO network: updated network environment: gateway [192.168.1.1]",
            ],
        );
        let kinds: Vec<Kind> = events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![Kind::Listening, Kind::Interface, Kind::Started]);
        assert_eq!(events[0].target.as_deref(), Some("127.0.0.1:33080"));
        assert_eq!(events[1].detail.as_deref(), Some("Ethernet"));
        assert_eq!(events[2].detail.as_deref(), Some("0.08s"));
        assert_eq!(events[2].time, "16:42:48");
    }

    #[test]
    fn successful_connections_stay_silent() {
        let mut parser = LogParser::default();
        let events = feed(
            &mut parser,
            &[
                "+0300 2026-10-13 16:42:50 INFO [1524510185 0ms] inbound/mixed[in]: inbound connection from 127.0.0.1:54884",
                "+0300 2026-10-13 16:42:50 INFO [1524510185 1ms] inbound/mixed[in]: inbound connection to ifconfig.me:443",
                "+0300 2026-10-13 16:42:50 INFO [1524510185 4ms] router: found process path: C:\\Windows\\System32\\curl.exe",
                "+0300 2026-10-13 16:42:50 INFO [1524510185 7ms] outbound/vless[proxy]: outbound connection to ifconfig.me:443",
            ],
        );
        assert!(events.is_empty());
    }

    #[test]
    fn failure_carries_app_domain_and_route_from_context() {
        let mut parser = LogParser::default();
        let events = feed(
            &mut parser,
            &[
                "+0300 2026-10-13 16:43:10 INFO [208626657 1ms] inbound/tun[tun]: inbound connection to 149.154.167.35:443",
                "+0300 2026-10-13 16:43:10 INFO [208626657 1ms] router: sniffed protocol: tls, domain: web.telegram.org",
                "+0300 2026-10-13 16:43:10 INFO [208626657 1ms] router: found process path: C:\\Users\\me\\AppData\\Roaming\\Telegram Desktop\\Telegram.exe",
                "+0300 2026-10-13 16:43:10 INFO [208626657 3ms] outbound/direct[direct]: outbound connection to 149.154.167.35:443",
                "+0300 2026-10-13 16:43:15 ERROR [208626657 5.0s] connection: open connection to 149.154.167.35:443 using outbound/direct[direct]: dial tcp 149.154.167.35:443: i/o timeout",
            ],
        );
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.kind, Kind::ConnectionFailed);
        assert_eq!(event.level, Level::Error);
        assert_eq!(event.app.as_deref(), Some("Telegram.exe"));
        assert_eq!(event.domain.as_deref(), Some("web.telegram.org"));
        assert_eq!(event.target.as_deref(), Some("149.154.167.35:443"));
        assert_eq!(event.route, Some(Route::Direct));
        assert_eq!(event.reason.as_deref(), Some("i/o timeout"));
        assert!(parser.connections.is_empty());
    }

    #[test]
    fn failure_without_context_still_names_target_and_route() {
        let mut parser = LogParser::default();
        let event = parser
            .ingest("+0300 2026-10-13 16:32:10 ERROR [4011279275 5.1s] connection: open connection to 149.154.167.35:80 using outbound/direct[direct]: dial tcp 149.154.167.35:80: i/o timeout")
            .unwrap();
        assert_eq!(event.route, Some(Route::Direct));
        assert_eq!(event.target.as_deref(), Some("149.154.167.35:80"));
        assert_eq!(event.domain, None);
        assert_eq!(event.app, None);
    }

    #[test]
    fn foreign_lines_pass_through() {
        let mut parser = LogParser::default();
        let event = parser.ingest("panic: something broke").unwrap();
        assert_eq!(event.kind, Kind::Other);
        assert_eq!(event.raw, "panic: something broke");
    }

    #[test]
    fn tracked_connections_are_bounded() {
        let mut parser = LogParser::default();
        for id in 0..(MAX_TRACKED_CONNECTIONS as u64 + 50) {
            parser.ingest(&format!(
                "+0300 2026-10-13 16:42:50 INFO [{id} 0ms] inbound/mixed[in]: inbound connection to a.com:443"
            ));
        }
        assert_eq!(parser.connections.len(), MAX_TRACKED_CONNECTIONS);
    }
}
