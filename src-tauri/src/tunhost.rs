//! Elevated supervisor for the core in TUN mode.
//!
//! A TUN interface needs administrator (Windows) or root (macOS). Rather than
//! elevating the whole app, Routeway relaunches its own binary elevated with
//! `--tun-host`, and that process owns sing-box. The unprivileged app talks to it
//! through files in the config directory, so stopping or reloading never needs a
//! second password prompt, and the host shuts the core down by itself if the app
//! dies.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const HOST_FLAG: &str = "--tun-host";

pub struct HostPaths {
    pub config: PathBuf,
    pub log: PathBuf,
    pub pid: PathBuf,
    pub stop: PathBuf,
    pub reload: PathBuf,
}

impl HostPaths {
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            config: dir.join("sing-box.json"),
            log: dir.join("tun.log"),
            pid: dir.join("tun-host.pid"),
            stop: dir.join("tun.stop"),
            reload: dir.join("tun.reload"),
        }
    }

    pub fn host_pid(&self) -> Option<u32> {
        fs::read_to_string(&self.pid).ok()?.trim().parse().ok()
    }
}

fn core_binary() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|err| t!("err.self_exe", err = err))?;
    let dir = exe
        .parent()
        .ok_or_else(|| t!("err.config_dir_plain"))?;
    Ok(dir.join(if cfg!(windows) { "sing-box.exe" } else { "sing-box" }))
}

fn spawn_core(core: &Path, paths: &HostPaths, truncate_log: bool) -> Result<Child, String> {
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .append(!truncate_log)
        .truncate(truncate_log)
        .open(&paths.log)
        .map_err(|err| t!("err.write_file", path = paths.log.display(), err = err))?;
    let log_err = log
        .try_clone()
        .map_err(|err| t!("err.write_file", path = paths.log.display(), err = err))?;

    let mut command = Command::new(core);
    command
        .arg("run")
        .arg("--disable-color")
        .arg("-c")
        .arg(&paths.config)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
        .spawn()
        .map_err(|err| t!("err.core_spawn", err = err))
}

fn append_log(path: &Path, line: &str) {
    use std::io::Write;
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn halt(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub fn run_host(dir: PathBuf, parent: u32) -> i32 {
    let paths = HostPaths::in_dir(&dir);
    let _ = fs::remove_file(&paths.stop);
    let _ = fs::remove_file(&paths.reload);
    let _ = File::create(&paths.log);

    let spawned = core_binary().and_then(|core| spawn_core(&core, &paths, true).map(|c| (core, c)));
    let (core, mut child) = match spawned {
        Ok(pair) => pair,
        Err(err) => {
            append_log(&paths.log, &err);
            return 1;
        }
    };
    let _ = fs::write(&paths.pid, std::process::id().to_string());

    let code = loop {
        std::thread::sleep(Duration::from_millis(250));

        if let Ok(Some(status)) = child.try_wait() {
            break status.code().unwrap_or(1);
        }
        if paths.stop.exists() || !is_alive(parent) {
            halt(&mut child);
            break 0;
        }
        if paths.reload.exists() {
            halt(&mut child);
            let respawned = spawn_core(&core, &paths, false);
            let _ = fs::remove_file(&paths.reload);
            match respawned {
                Ok(next) => child = next,
                Err(err) => {
                    append_log(&paths.log, &err);
                    break 1;
                }
            }
        }
    };

    let _ = fs::remove_file(&paths.stop);
    let _ = fs::remove_file(&paths.pid);
    code
}

pub fn request_stop(paths: &HostPaths) {
    let Some(pid) = paths.host_pid() else { return };
    if !is_alive(pid) {
        let _ = fs::remove_file(&paths.pid);
        return;
    }
    let _ = fs::write(&paths.stop, b"");
    let deadline = Instant::now() + Duration::from_secs(5);
    while is_alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn request_reload(paths: &HostPaths) -> Result<(), String> {
    fs::write(&paths.reload, b"").map_err(|err| t!("err.core_restart", err = err))?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while paths.reload.exists() {
        if Instant::now() > deadline {
            return Err(t!("err.core_no_reload"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Blocks while the OS shows the elevation prompt.
pub fn launch(dir: &Path) -> Result<u32, String> {
    let paths = HostPaths::in_dir(dir);
    let _ = fs::remove_file(&paths.pid);

    imp::launch_elevated(dir, std::process::id())?;

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(pid) = paths.host_pid() {
            return Ok(pid);
        }
        if Instant::now() > deadline {
            let log = fs::read_to_string(&paths.log).unwrap_or_default();
            let detail = log.trim();
            return Err(if detail.is_empty() {
                t!("err.tun_failed")
            } else {
                t!("err.tun_failed_detail", detail = detail)
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub fn is_alive(pid: u32) -> bool {
    imp::is_alive(pid)
}

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_CANCELLED, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn launch_elevated(dir: &Path, parent: u32) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|err| t!("err.self_exe", err = err))?;
        let dir = dir.to_string_lossy();
        if dir.contains('"') {
            return Err(t!("err.unsupported_path", path = dir));
        }
        let params = format!(
            "{} \"{}\" {parent}",
            super::HOST_FLAG,
            dir.trim_end_matches('\\')
        );

        let verb = wide("runas".as_ref());
        let file = wide(exe.as_os_str());
        let params = wide(params.as_ref());

        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;

        if unsafe { ShellExecuteExW(&mut info) } != 0 {
            return Ok(());
        }
        let code = unsafe { GetLastError() };
        if code == ERROR_CANCELLED {
            return Err(t!("err.cancelled"));
        }
        Err(t!("err.elevation_code", code = code))
    }

    pub fn is_alive(pid: u32) -> bool {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, 0, pid);
            if handle.is_null() {
                return false;
            }
            let alive = WaitForSingleObject(handle, 0) == WAIT_TIMEOUT;
            CloseHandle(handle);
            alive
        }
    }
}

#[cfg(unix)]
mod imp {
    use std::path::Path;
    use std::process::Command;

    fn quoted(value: &str) -> Result<String, String> {
        if value.contains('\'') || value.contains('"') || value.contains('\\') {
            return Err(t!("err.unsupported_path", path = value));
        }
        Ok(format!("'{value}'"))
    }

    pub fn launch_elevated(dir: &Path, parent: u32) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|err| t!("err.self_exe", err = err))?;
        let command = format!(
            "{} {} {} {parent} > /dev/null 2>&1 &",
            quoted(&exe.to_string_lossy())?,
            super::HOST_FLAG,
            quoted(&dir.to_string_lossy())?,
        );
        let prompt = crate::i18n::tr("tun.prompt");
        let script = format!(
            "do shell script \"{command}\" with prompt \"{prompt}\" with administrator privileges"
        );
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
        Err(t!("err.elevation", err = stderr.trim()))
    }

    pub fn is_alive(pid: u32) -> bool {
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };
        if unsafe { libc::kill(pid, 0) } == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}
