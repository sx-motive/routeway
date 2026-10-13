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
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    /// Whether the system proxy should point at us while the core runs.
    #[serde(default)]
    pub system_proxy: bool,
    #[serde(default)]
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: None,
            domains: Vec::new(),
            listen_port: default_listen_port(),
            system_proxy: true,
            autostart: false,
        }
    }
}

impl Settings {
    /// Domains cleaned up for use as sing-box `domain_suffix` entries: trimmed,
    /// lowercased, scheme/path/leading-dot stripped, deduplicated.
    pub fn normalized_domains(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for raw in &self.domains {
            if let Some(domain) = normalize_domain(raw) {
                if !out.contains(&domain) {
                    out.push(domain);
                }
            }
        }
        out
    }
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
        .map_err(|err| format!("не удалось определить каталог настроек: {err}"))?;
    fs::create_dir_all(&dir).map_err(|err| format!("не удалось создать {dir:?}: {err}"))?;
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
    fs::write(&path, raw).map_err(|err| format!("не удалось сохранить снимок прокси: {err}"))
}

/// Reads the snapshot and removes it — restoring is a one-shot operation.
pub fn take_proxy_backup(app: &AppHandle) -> Option<String> {
    let path = proxy_backup_path(app).ok()?;
    let raw = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    Some(raw)
}

pub fn load(app: &AppHandle) -> Settings {
    let Ok(path) = settings_path(app) else {
        return Settings::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Settings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    let body = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("не удалось сериализовать настройки: {err}"))?;
    fs::write(&path, body).map_err(|err| format!("не удалось записать {path:?}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::normalize_domain;

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
