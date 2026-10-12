//! System proxy wiring.
//!
//! macOS needs administrator rights to touch network preferences, so changes go
//! through one authorized `osascript` call instead of many prompts. Windows keeps
//! proxy settings in the per-user registry, so no elevation is needed there — but
//! WinINet has to be told the settings changed.

#[cfg(target_os = "macos")]
mod imp {
    use std::process::Command;

    const BYPASS: &str = "localhost 127.0.0.1 ::1 *.local";

    fn primary_service() -> Result<String, String> {
        let global = scutil("show State:/Network/Global/IPv4")?;
        let service_id = global
            .lines()
            .find_map(|line| {
                let line = line.trim();
                line.strip_prefix("PrimaryService :").map(|v| v.trim().to_string())
            })
            .ok_or_else(|| "нет активного сетевого подключения".to_string())?;

        let setup = scutil(&format!("show Setup:/Network/Service/{service_id}"))?;
        let name = setup
            .lines()
            .find_map(|line| {
                let line = line.trim();
                line.strip_prefix("UserDefinedName :").map(|v| v.trim().to_string())
            })
            .ok_or_else(|| "не удалось определить имя сетевого сервиса".to_string())?;

        if name.contains('\'') || name.contains('"') || name.contains('\\') {
            return Err(format!("неподдерживаемое имя сетевого сервиса: {name}"));
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
            .map_err(|err| format!("не удалось запустить scutil: {err}"))?;
        child
            .stdin
            .as_mut()
            .ok_or_else(|| "scutil: нет stdin".to_string())?
            .write_all(query.as_bytes())
            .map_err(|err| format!("scutil: {err}"))?;
        let out = child
            .wait_with_output()
            .map_err(|err| format!("scutil: {err}"))?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    fn run_as_admin(shell_command: &str) -> Result<(), String> {
        let escaped = shell_command.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("do shell script \"{escaped}\" with administrator privileges");
        let out = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|err| format!("не удалось запустить osascript: {err}"))?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("-128") {
            return Err("отменено пользователем".into());
        }
        Err(format!("не удалось изменить системный прокси: {}", stderr.trim()))
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

    /// Whether any system proxy is set, regardless of which port it points at.
    pub fn is_enabled() -> bool {
        read_https_proxy()
            .map(|text| text.lines().any(|l| l.trim() == "Enabled: Yes"))
            .unwrap_or(false)
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
            .map_err(|err| format!("не удалось открыть настройки интернета: {err}"))
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

    pub fn enable(port: u16) -> Result<(), String> {
        let key = open(KEY_READ | KEY_WRITE)?;
        key.set_value("ProxyServer", &format!("127.0.0.1:{port}"))
            .map_err(|err| format!("не удалось записать ProxyServer: {err}"))?;
        key.set_value("ProxyOverride", &BYPASS.to_string())
            .map_err(|err| format!("не удалось записать ProxyOverride: {err}"))?;
        key.set_value("ProxyEnable", &1u32)
            .map_err(|err| format!("не удалось включить прокси: {err}"))?;
        notify();
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let key = open(KEY_READ | KEY_WRITE)?;
        key.set_value("ProxyEnable", &0u32)
            .map_err(|err| format!("не удалось выключить прокси: {err}"))?;
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

    /// Whether any system proxy is set, regardless of which port it points at.
    pub fn is_enabled() -> bool {
        let Ok(key) = open(KEY_READ) else {
            return false;
        };
        key.get_value::<u32, _>("ProxyEnable").unwrap_or(0) == 1
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn enable(_port: u16) -> Result<(), String> {
        Err("системный прокси поддерживается только на macOS и Windows".into())
    }
    pub fn disable() -> Result<(), String> {
        Err("системный прокси поддерживается только на macOS и Windows".into())
    }
    pub fn is_active(_port: u16) -> bool {
        false
    }
    pub fn is_enabled() -> bool {
        false
    }
}

pub use imp::{disable, enable, is_active, is_enabled};
