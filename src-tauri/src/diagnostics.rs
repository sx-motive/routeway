//! Read-only process inventory. Process control stays with the core supervisor;
//! this module never kills processes by executable name or a frontend-supplied PID.

use std::collections::HashSet;

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Clone, Serialize)]
pub struct ProcessInfo {
    pid: u32,
    parent_pid: u32,
    name: String,
    role: String,
}

#[derive(Serialize)]
pub struct Diagnostics {
    processes: Vec<ProcessInfo>,
    core_running: bool,
    system_proxy_active: bool,
    tun_active: bool,
    listen_port: u16,
}

pub fn snapshot(app: &AppHandle) -> Result<Diagnostics, String> {
    let settings = crate::settings::load(app);
    let state = app.state::<crate::core::CoreState>();
    let processes = related_processes(
        all_processes()?,
        std::process::id(),
        &crate::core::diagnostic_pids(app),
    );
    Ok(Diagnostics {
        processes,
        core_running: state.is_running(),
        system_proxy_active: crate::sysproxy::is_active(settings.listen_port),
        tun_active: state.is_tun(),
        listen_port: settings.listen_port,
    })
}

fn related_processes(
    mut all: Vec<ProcessInfo>,
    app_pid: u32,
    recorded: &[(u32, &str)],
) -> Vec<ProcessInfo> {
    let mut included = HashSet::from([app_pid]);
    for process in &mut all {
        let name = process.name.to_lowercase();
        if process.pid == app_pid {
            process.role = "app".into();
        } else if let Some((_, role)) = recorded.iter().find(|(pid, role)| {
            *pid == process.pid
                && match *role {
                    "core" => name.contains("sing-box"),
                    "host" => name.contains("routeway"),
                    _ => false,
                }
        }) {
            process.role = (*role).into();
            included.insert(process.pid);
        }
    }
    // Several passes handle arbitrary OS enumeration order and nested WebViews.
    loop {
        let mut changed = false;
        for process in &all {
            if included.contains(&process.parent_pid) && included.insert(process.pid) {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    all.retain(|process| included.contains(&process.pid));
    for process in &mut all {
        if process.role.is_empty() {
            let name = process.name.to_lowercase();
            process.role = if name.contains("sing-box") {
                "core"
            } else if name.contains("webview") || name.contains("webkit") {
                "webview"
            } else {
                "child"
            }
            .into();
        }
    }
    all.sort_by_key(|process| (process.pid != app_pid, process.pid));
    all
}

#[cfg(windows)]
fn all_processes() -> Result<Vec<ProcessInfo>, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    unsafe {
        let handle = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if handle == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut processes = Vec::new();
        let mut found = Process32FirstW(handle, &mut entry);
        while found != 0 {
            let len = entry
                .szExeFile
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(entry.szExeFile.len());
            processes.push(ProcessInfo {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                name: String::from_utf16_lossy(&entry.szExeFile[..len]),
                role: String::new(),
            });
            found = Process32NextW(handle, &mut entry);
        }
        CloseHandle(handle);
        Ok(processes)
    }
}

#[cfg(not(windows))]
fn all_processes() -> Result<Vec<ProcessInfo>, String> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,comm="])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            let parent_pid = fields.next()?.parse().ok()?;
            let name = fields.collect::<Vec<_>>().join(" ");
            Some(ProcessInfo {
                pid,
                parent_pid,
                name,
                role: String::new(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, parent_pid: u32, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid,
            name: name.into(),
            role: String::new(),
        }
    }

    #[test]
    fn includes_nested_children_and_elevated_host_but_not_other_apps() {
        let result = related_processes(
            vec![
                process(12, 11, "msedgewebview2.exe"),
                process(11, 10, "msedgewebview2.exe"),
                process(10, 1, "routeway.exe"),
                process(20, 1, "routeway.exe"),
                process(21, 20, "sing-box.exe"),
                process(30, 1, "sing-box.exe"),
                process(40, 1, "unrelated.exe"),
            ],
            10,
            &[(20, "host"), (40, "core")],
        );
        assert_eq!(
            result.iter().map(|p| p.pid).collect::<Vec<_>>(),
            [10, 11, 12, 20, 21]
        );
        assert_eq!(result[3].role, "host");
        assert_eq!(result[4].role, "core");
    }

    #[test]
    fn process_inventory_contains_this_process() {
        let all = all_processes().unwrap();
        assert!(all.iter().any(|p| p.pid == std::process::id()));
    }
}
