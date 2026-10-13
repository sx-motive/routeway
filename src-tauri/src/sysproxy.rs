//! System proxy wiring.
//!
//! macOS needs administrator rights to touch network preferences, so changes go
//! through one authorized `osascript` call instead of many prompts. Windows keeps
//! proxy settings in the per-user registry, so no elevation is needed there — but
//! WinINet has to be told the settings changed.

#[cfg(target_os = "macos")]
mod imp {
    use std::process::Command;

    use serde::{Deserialize, Serialize};

    const BYPASS: &str = "localhost 127.0.0.1 ::1 *.local";

    /// What a single proxy slot looked like before Routeway touched it.
    #[derive(Debug, Default, Serialize, Deserialize)]
    struct Entry {
        enabled: bool,
        server: String,
        port: u16,
    }

    #[derive(Debug, Default, Serialize, Deserialize)]
    struct Snapshot {
        service: String,
        web: Entry,
        secure: Entry,
        socks: Entry,
        bypass: Vec<String>,
    }

    fn read_entry(service: &str, getter: &str) -> Entry {
        let Ok(out) = Command::new("/usr/sbin/networksetup")
            .args([getter, service])
            .output()
        else {
            return Entry::default();
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let mut entry = Entry::default();
        for line in text.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("Enabled:") {
                entry.enabled = v.trim() == "Yes";
            } else if let Some(v) = line.strip_prefix("Server:") {
                entry.server = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("Port:") {
                entry.port = v.trim().parse().unwrap_or(0);
            }
        }
        entry
    }

    fn read_bypass(service: &str) -> Vec<String> {
        let Ok(out) = Command::new("/usr/sbin/networksetup")
            .args(["-getproxybypassdomains", service])
            .output()
        else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| {
                !l.is_empty()
                    // networksetup reports absence as a sentence, not an empty list.
                    && !l.starts_with("There aren't any")
                    && !l.contains(' ')
            })
            .collect()
    }

    /// Records the current proxy configuration so it can be put back verbatim.
    pub fn capture() -> Option<String> {
        let service = primary_service().ok()?;
        let snapshot = Snapshot {
            web: read_entry(&service, "-getwebproxy"),
            secure: read_entry(&service, "-getsecurewebproxy"),
            socks: read_entry(&service, "-getsocksfirewallproxy"),
            bypass: read_bypass(&service),
            service,
        };
        serde_json::to_string(&snapshot).ok()
    }

    pub fn restore(raw: &str) -> Result<(), String> {
        let snapshot: Snapshot = serde_json::from_str(raw)
            .map_err(|err| t!("err.read_snapshot", err = err))?;

        // Prefer the service recorded in the snapshot, but fall back to whatever
        // is primary now — the user may have switched Wi-Fi for Ethernet since.
        let service = if snapshot.service.is_empty() {
            primary_service()?
        } else {
            snapshot.service.clone()
        };
        if service.contains('\'') || service.contains('"') || service.contains('\\') {
            return Err(t!("err.network_service_name", name = service));
        }

        let ns = "/usr/sbin/networksetup";
        let mut parts: Vec<String> = Vec::new();
        for (entry, set, state) in [
            (&snapshot.web, "-setwebproxy", "-setwebproxystate"),
            (&snapshot.secure, "-setsecurewebproxy", "-setsecurewebproxystate"),
            (
                &snapshot.socks,
                "-setsocksfirewallproxy",
                "-setsocksfirewallproxystate",
            ),
        ] {
            if entry.enabled && !entry.server.is_empty() {
                parts.push(format!(
                    "{ns} {set} '{service}' {} {}",
                    entry.server, entry.port
                ));
            } else {
                parts.push(format!("{ns} {state} '{service}' off"));
            }
        }
        let bypass = if snapshot.bypass.is_empty() {
            "''".to_string()
        } else {
            snapshot.bypass.join(" ")
        };
        parts.push(format!("{ns} -setproxybypassdomains '{service}' {bypass}"));

        run_as_admin(&parts.join(" && "))
    }

    fn primary_service() -> Result<String, String> {
        let global = scutil("show State:/Network/Global/IPv4")?;
        let service_id = global
            .lines()
            .find_map(|line| {
                let line = line.trim();
                line.strip_prefix("PrimaryService :").map(|v| v.trim().to_string())
            })
            .ok_or_else(|| t!("err.no_network"))?;

        let setup = scutil(&format!("show Setup:/Network/Service/{service_id}"))?;
        let name = setup
            .lines()
            .find_map(|line| {
                let line = line.trim();
                line.strip_prefix("UserDefinedName :").map(|v| v.trim().to_string())
            })
            .ok_or_else(|| t!("err.network_service"))?;

        if name.contains('\'') || name.contains('"') || name.contains('\\') {
            return Err(t!("err.network_service_name", name = name));
        }
        Ok(name)
    }

    fn scutil(query: &str) -> Result<String, String> {
        use std::io::Write;
        use std::process::Stdio;

        let mut child = Command::new("/usr/sbin/scutil")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| t!("err.scutil", err = err))?;
        child
            .stdin
            .as_mut()
            .ok_or_else(|| t!("err.scutil", err = "stdin"))?
            .write_all(query.as_bytes())
            .map_err(|err| t!("err.scutil", err = err))?;
        let out = child
            .wait_with_output()
            .map_err(|err| t!("err.scutil", err = err))?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    fn run_as_admin(shell_command: &str) -> Result<(), String> {
        let escaped = shell_command.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("do shell script \"{escaped}\" with administrator privileges");
        let out = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|err| t!("err.osascript", err = err))?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("-128") {
            return Err(t!("err.cancelled"));
        }
        Err(t!("err.change_proxy", err = stderr.trim()))
    }

    pub fn enable(port: u16) -> Result<(), String> {
        let service = primary_service()?;
        let ns = "/usr/sbin/networksetup";
        let command = format!(
            "{ns} -setwebproxy '{service}' 127.0.0.1 {port} && \
             {ns} -setsecurewebproxy '{service}' 127.0.0.1 {port} && \
             {ns} -setsocksfirewallproxy '{service}' 127.0.0.1 {port} && \
             {ns} -setproxybypassdomains '{service}' {BYPASS}"
        );
        run_as_admin(&command)
    }

    pub fn disable() -> Result<(), String> {
        let service = primary_service()?;
        let ns = "/usr/sbin/networksetup";
        let command = format!(
            "{ns} -setwebproxystate '{service}' off && \
             {ns} -setsecurewebproxystate '{service}' off && \
             {ns} -setsocksfirewallproxystate '{service}' off"
        );
        run_as_admin(&command)
    }

    fn read_https_proxy() -> Option<String> {
        let service = primary_service().ok()?;
        let out = Command::new("/usr/sbin/networksetup")
            .args(["-getsecurewebproxy", &service])
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn is_active(port: u16) -> bool {
        let Some(text) = read_https_proxy() else {
            return false;
        };
        let enabled = text.lines().any(|l| l.trim() == "Enabled: Yes");
        let right_port = text.lines().any(|l| l.trim() == format!("Port: {port}"));
        enabled && right_port
    }

}

#[cfg(target_os = "windows")]
mod imp {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const SETTINGS_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    const BYPASS: &str = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.2*;172.30.*;172.31.*;192.168.*;<local>";

    fn open(access: u32) -> Result<RegKey, String> {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(SETTINGS_KEY, access)
            .map_err(|err| t!("err.open_inet_settings", err = err))
    }

    /// WinINet caches proxy settings; without this notification browsers keep
    /// using the previous values until restart.
    fn notify() {
        use windows_sys::Win32::Networking::WinInet::{
            InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
        };
        unsafe {
            InternetSetOptionW(std::ptr::null_mut(), INTERNET_OPTION_SETTINGS_CHANGED, std::ptr::null_mut(), 0);
            InternetSetOptionW(std::ptr::null_mut(), INTERNET_OPTION_REFRESH, std::ptr::null_mut(), 0);
        }
    }

    #[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
    struct Snapshot {
        enable: u32,
        server: String,
        bypass: String,
    }

    /// Records the current proxy configuration so it can be put back verbatim.
    pub fn capture() -> Option<String> {
        let key = open(KEY_READ).ok()?;
        let snapshot = Snapshot {
            enable: key.get_value("ProxyEnable").unwrap_or(0),
            server: key.get_value("ProxyServer").unwrap_or_default(),
            bypass: key.get_value("ProxyOverride").unwrap_or_default(),
        };
        serde_json::to_string(&snapshot).ok()
    }

    pub fn restore(raw: &str) -> Result<(), String> {
        let snapshot: Snapshot = serde_json::from_str(raw)
            .map_err(|err| t!("err.read_snapshot", err = err))?;
        let key = open(KEY_READ | KEY_WRITE)?;
        key.set_value("ProxyServer", &snapshot.server)
            .map_err(|err| t!("err.registry_write", name = "ProxyServer", err = err))?;
        key.set_value("ProxyOverride", &snapshot.bypass)
            .map_err(|err| t!("err.registry_write", name = "ProxyOverride", err = err))?;
        key.set_value("ProxyEnable", &snapshot.enable)
            .map_err(|err| t!("err.registry_write", name = "ProxyEnable", err = err))?;
        notify();
        Ok(())
    }

    pub fn enable(port: u16) -> Result<(), String> {
        let key = open(KEY_READ | KEY_WRITE)?;
        key.set_value("ProxyServer", &format!("127.0.0.1:{port}"))
            .map_err(|err| t!("err.registry_write", name = "ProxyServer", err = err))?;
        key.set_value("ProxyOverride", &BYPASS.to_string())
            .map_err(|err| t!("err.registry_write", name = "ProxyOverride", err = err))?;
        key.set_value("ProxyEnable", &1u32)
            .map_err(|err| t!("err.registry_write", name = "ProxyEnable", err = err))?;
        notify();
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let key = open(KEY_READ | KEY_WRITE)?;
        key.set_value("ProxyEnable", &0u32)
            .map_err(|err| t!("err.registry_write", name = "ProxyEnable", err = err))?;
        notify();
        Ok(())
    }

    pub fn is_active(port: u16) -> bool {
        let Ok(key) = open(KEY_READ) else {
            return false;
        };
        let enabled: u32 = key.get_value("ProxyEnable").unwrap_or(0);
        let server: String = key.get_value("ProxyServer").unwrap_or_default();
        enabled == 1 && server == format!("127.0.0.1:{port}")
    }

}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn enable(_port: u16) -> Result<(), String> {
        Err(t!("err.proxy_unsupported"))
    }
    pub fn disable() -> Result<(), String> {
        Err(t!("err.proxy_unsupported"))
    }
    pub fn is_active(_port: u16) -> bool {
        false
    }
    pub fn capture() -> Option<String> {
        None
    }
    pub fn restore(_raw: &str) -> Result<(), String> {
        Err(t!("err.proxy_unsupported"))
    }
}

pub use imp::{capture, disable, enable, is_active, restore};
