//! Windows session shutdown is separate from closing a tray window.
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
         WindowsAndMessaging::{WM_ENDSESSION, WM_NCDESTROY, WM_QUERYENDSESSION}},
};

const SUBCLASS_ID: usize = 0x52575344;
static CLEANUP_STARTED: AtomicBool = AtomicBool::new(false);

pub fn is_ending() -> bool {
    CLEANUP_STARTED.load(Ordering::Acquire)
}

pub fn install(window: &WebviewWindow) -> tauri::Result<()> {
    let hwnd = window.hwnd()?.0 as HWND;
    let app = Box::into_raw(Box::new(window.app_handle().clone()));
    // Setup runs on the window's owning thread. The boxed handle lives until
    // WM_NCDESTROY, when the subclass is removed and the allocation released.
    if unsafe { SetWindowSubclass(hwnd, Some(session_proc), SUBCLASS_ID, app as usize) } == 0 {
        unsafe { drop(Box::from_raw(app)); }
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

fn session_ending(message: u32, confirmed: usize) -> bool {
    message == WM_ENDSESSION && confirmed != 0
}

unsafe extern "system" fn session_proc(
    hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM,
    id: usize, data: usize,
) -> LRESULT {
    if message == WM_QUERYENDSESSION {
        // Never veto shutdown; cancellation must leave connections untouched.
        return 1;
    }
    if session_ending(message, wparam) {
        let app = &*(data as *const AppHandle);
        finish_session(app.clone());
        return 0;
    }
    if message == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(session_proc), id);
        drop(Box::from_raw(data as *mut AppHandle));
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

fn finish_session(app: AppHandle) {
    if CLEANUP_STARTED.swap(true, Ordering::AcqRel) { return; }
    let (done, finished) = mpsc::channel();
    // Cleanup may wait on a connection operation or an elevated TUN host.
    // Keep the window message bounded below Windows' five-second deadline.
    let _ = std::thread::Builder::new().name("session-shutdown".into()).spawn(move || {
        tauri::async_runtime::block_on(async {
            crate::recovery::cancel(&app);
            let settings = crate::settings::load(&app);
            // Restore proxy first, even if another operation holds the lock.
            crate::release_system_proxy(&app, settings.listen_port).await;
            let ssh_app = app.clone();
            let ssh = tauri::async_runtime::spawn_blocking(move || {
                ssh_app.state::<crate::ssh::SshState>().stop_all();
            });
            let connection = app.state::<crate::ConnectionState>();
            let _guard = connection.0.lock().await;
            crate::teardown(&app, &settings).await;
            let _ = ssh.await;
        });
        let _ = done.send(());
    });
    let _ = finished.recv_timeout(Duration::from_millis(3500));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_only_runs_for_confirmed_session_end() {
        assert!(!session_ending(WM_QUERYENDSESSION, 1));
        assert!(!session_ending(WM_ENDSESSION, 0));
        assert!(session_ending(WM_ENDSESSION, 1));
        assert!(!session_ending(WM_NCDESTROY, 1));
    }
}
