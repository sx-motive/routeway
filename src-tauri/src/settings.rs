use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// A VLESS + REALITY endpoint. v1 supports exactly this protocol because it is
/// what the author's servers run; the shape is deliberately flat so the UI can
/// bind to it field by field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Server {
    #[serde(default)]
    pub label: String,
    pub address: String,
    pub port: u16,
    pub uuid: String,
    #[serde(default)]
    pub flow: String,
    #[serde(default)]
    pub sni: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub short_id: String,
    #[serde(default = "default_fingerprint")]
    pub fingerprint: String,
}

fn default_fingerprint() -> String {
    "chrome".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_listen_port() -> u16 {
    2080
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub server: Option<Server>,
    /// Domains routed through the server. Everything else stays direct.
    #[serde(default)]
    pub domains: Vec<String>,
    /// Processes routed through the server, by executable name ("codex") or by
    /// full path ("/usr/bin/curl"). Matching works for any traffic that reaches
    /// our local inbound — no TUN and no elevated privileges required.
    #[serde(default)]
    pub processes: Vec<String>,
    /// Domains that must never reach the core, e.g. split-horizon corporate names
    /// that resolve to internal addresses only through the VPN's own DNS.
    #[serde(default)]
    pub direct: Vec<String>,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    /// Whether the system proxy should point at us while the core runs.
    #[serde(default)]
    pub system_proxy: bool,
    #[serde(default)]
    pub tun: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: None,
            domains: Vec::new(),
            processes: Vec::new(),
            direct: Vec::new(),
            listen_port: default_listen_port(),
            system_proxy: true,
            tun: false,
            language: default_language(),
            autostart: false,
        }
    }
}

impl Settings {
    /// Domains cleaned up for use as sing-box `domain_suffix` entries: trimmed,
    /// lowercased, scheme/path/leading-dot stripped, deduplicated.
    pub fn normalized_domains(&self) -> Vec<String> {
        normalize_domains(&self.domains)
    }

    pub fn normalized_direct(&self) -> Vec<String> {
        normalize_domains(&self.direct)
    }

    /// Process entries split into the two shapes sing-box matches on: bare
    /// executable names and absolute paths.
    pub fn process_rules(&self) -> (Vec<String>, Vec<String>) {
        let mut names: Vec<String> = Vec::new();
        let mut paths: Vec<String> = Vec::new();
        for raw in &self.processes {
            let value = raw.trim();
            if value.is_empty() || value.starts_with('#') {
                continue;
            }
            let target = if value.contains('/') || value.contains('\\') {
                &mut paths
            } else {
                &mut names
            };
            let value = value.to_string();
            if !target.contains(&value) {
                target.push(value);
            }
        }
        (names, paths)
    }
}

fn normalize_domains(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for entry in raw {
        if let Some(domain) = normalize_domain(entry) {
            if !out.contains(&domain) {
                out.push(domain);
            }
        }
    }
    out
}

pub fn normalize_domain(raw: &str) -> Option<String> {
    let mut value = raw.trim().to_lowercase();
    if value.is_empty() || value.starts_with('#') {
        return None;
    }
    if let Some(idx) = value.find("://") {
        value = value[idx + 3..].to_string();
    }
    value = value
        .split('/')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    // strip credentials and port
    if let Some(idx) = value.rfind('@') {
        value = value[idx + 1..].to_string();
    }
    if let Some(idx) = value.find(':') {
        value = value[..idx].to_string();
    }
    value = value.trim_matches('.').to_string();
    if value.is_empty() || !value.contains('.') {
        return None;
    }
    Some(value)
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|err| t!("err.config_dir", err = err))?;
    fs::create_dir_all(&dir).map_err(|err| t!("err.create_dir", path = dir.display(), err = err))?;
    Ok(dir)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("settings.json"))
}

/// Where the pre-Routeway system proxy configuration is parked while we own it.
fn proxy_backup_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("system-proxy-backup.json"))
}

pub fn has_proxy_backup(app: &AppHandle) -> bool {
    proxy_backup_path(app).map(|p| p.exists()).unwrap_or(false)
}

pub fn save_proxy_backup(app: &AppHandle, raw: &str) -> Result<(), String> {
    let path = proxy_backup_path(app)?;
    fs::write(&path, raw).map_err(|err| t!("err.save_proxy_backup", err = err))
}

/// Reads the snapshot and removes it — restoring is a one-shot operation.
pub fn take_proxy_backup(app: &AppHandle) -> Option<String> {
    let path = proxy_backup_path(app).ok()?;
    let raw = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    Some(raw)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub direct: Vec<String>,
}

impl Rules {
    fn of(settings: &Settings) -> Self {
        Self {
            apps: settings.processes.clone(),
            domains: settings.domains.clone(),
            direct: settings.direct.clone(),
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: Rules = serde_json::from_str(text).map_err(|err| {
            t!(
                "err.rules_json",
                line = err.line(),
                column = err.column(),
                err = err
            )
        })?;

        let mut rejected: Vec<String> = Vec::new();
        let domains = parse_domains(&raw.domains, &mut rejected);
        let direct = parse_domains(&raw.direct, &mut rejected);
        if !rejected.is_empty() {
            return Err(t!("err.rules_domain", values = rejected.join(", ")));
        }

        let mut apps: Vec<String> = Vec::new();
        for entry in &raw.apps {
            let app = entry.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
            if !app.is_empty() && !apps.contains(&app) {
                apps.push(app);
            }
        }

        Ok(Self { apps, domains, direct })
    }

    fn to_text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

fn parse_domains(raw: &[String], rejected: &mut Vec<String>) -> Vec<String> {
    let mut domains: Vec<String> = Vec::new();
    for entry in raw {
        match normalize_domain(entry) {
            Some(domain) if !domains.contains(&domain) => domains.push(domain),
            Some(_) => {}
            None if entry.trim().is_empty() => {}
            None => rejected.push(entry.trim().to_string()),
        }
    }
    domains
}

fn rules_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("rules.json"))
}

pub fn ensure_rules_file(app: &AppHandle) -> Result<PathBuf, String> {
    let path = rules_path(app)?;
    if !path.exists() {
        save(app, &load(app))?;
    }
    Ok(path)
}

pub fn rules_text(app: &AppHandle) -> String {
    rules_path(app)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_else(|| Rules::of(&load(app)).to_text())
}

/// Rules live in their own file so they can be edited in bulk. Settings written
/// before that existed still carry the lists inline, which is what a missing
/// rules file falls back to.
pub fn load(app: &AppHandle) -> Settings {
    let mut settings = settings_path(app)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<Settings>(&raw).ok())
        .unwrap_or_default();

    let Ok(path) = rules_path(app) else {
        return settings;
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return settings;
    };
    match serde_json::from_str::<Rules>(&raw) {
        Ok(rules) => {
            settings.processes = rules.apps;
            settings.domains = rules.domains;
            settings.direct = rules.direct;
        }
        Err(_) => {
            // A hand-broken file would otherwise be overwritten by the next save.
            let _ = fs::copy(&path, path.with_file_name("rules.invalid.json"));
        }
    }
    settings
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let rules = rules_path(app)?;
    fs::write(&rules, Rules::of(settings).to_text())
        .map_err(|err| t!("err.write_file", path = rules.display(), err = err))?;

    let stored = Settings {
        processes: Vec::new(),
        domains: Vec::new(),
        direct: Vec::new(),
        ..settings.clone()
    };
    let path = settings_path(app)?;
    let body = serde_json::to_string_pretty(&stored)
        .map_err(|err| t!("err.serialize_settings", err = err))?;
    fs::write(&path, body).map_err(|err| t!("err.write_file", path = path.display(), err = err))
}

#[cfg(test)]
mod tests {
    use super::{normalize_domain, Rules};

    #[test]
    fn rules_normalize_and_dedupe() {
        let rules = Rules::parse(
            r#"{ "apps": [" codex ", "codex", ""], "domains": ["https://YouTube.com/x", "youtube.com", "  "] }"#,
        )
        .unwrap();
        assert_eq!(rules.apps, vec!["codex"]);
        assert_eq!(rules.domains, vec!["youtube.com"]);
    }

    #[test]
    fn direct_list_is_optional_and_normalized() {
        let rules = Rules::parse(
            r#"{ "direct": ["https://Jira.ArgusTelecom.ru/browse/X", "jira.argustelecom.ru"] }"#,
        )
        .unwrap();
        assert_eq!(rules.direct, vec!["jira.argustelecom.ru"]);
        assert!(Rules::parse(r#"{ "domains": [] }"#).unwrap().direct.is_empty());
        assert!(Rules::parse(r#"{ "direct": ["intranet"] }"#).is_err());
    }

    #[test]
    fn rules_reject_bad_json_with_position() {
        let err = Rules::parse("{\n  \"domains\": [\"a.com\",]\n}").unwrap_err();
        assert!(err.contains('2'), "{err}");
    }

    #[test]
    fn rules_reject_junk_domains_and_unknown_keys() {
        assert!(Rules::parse(r#"{ "domains": ["localhost"] }"#).is_err());
        assert!(Rules::parse(r#"{ "domain": ["a.com"] }"#).is_err());
    }

    #[test]
    fn strips_scheme_path_and_port() {
        assert_eq!(
            normalize_domain("https://www.YouTube.com/watch?v=1"),
            Some("www.youtube.com".into())
        );
        assert_eq!(normalize_domain("youtube.com:443"), Some("youtube.com".into()));
        assert_eq!(normalize_domain(".youtube.com."), Some("youtube.com".into()));
    }

    #[test]
    fn rejects_junk() {
        assert_eq!(normalize_domain("   "), None);
        assert_eq!(normalize_domain("# comment"), None);
        assert_eq!(normalize_domain("localhost"), None);
    }
}
