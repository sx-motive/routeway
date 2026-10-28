//! Independent OpenSSH supervisors. No shell, passwords, or proxy-core coupling.
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    io::{BufRead, BufReader},
    net::TcpListener,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Forward {
    pub label: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Host alias (or user@host) from the user's existing OpenSSH configuration.
    pub host: String,
    #[serde(default)]
    pub config_file: String,
    #[serde(default)]
    pub auto_connect: bool,
    pub forwards: Vec<Forward>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

impl Config {
    fn validate(&self) -> Result<(), String> {
        if self.profiles.len() > 50 {
            return Err("At most 50 SSH profiles are supported.".into());
        }
        let mut ids = HashSet::new();
        let mut ports = HashSet::new();
        for p in &self.profiles {
            if p.id.is_empty() || p.id.len() > 100 || !ids.insert(&p.id) {
                return Err("SSH profile IDs must be unique and nonempty.".into());
            }
            if p.name.trim().is_empty() || p.name.len() > 120 {
                return Err("Enter a profile name (up to 120 characters).".into());
            }
            if p.host.is_empty()
                || p.host.len() > 255
                || p.host.starts_with('-')
                || !p
                    .host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "@._-:".contains(c))
            {
                return Err(
                    "Enter an SSH host alias or user@hostname, without spaces or options.".into(),
                );
            }
            if !p.config_file.is_empty() && !PathBuf::from(&p.config_file).is_absolute() {
                return Err(
                    "SSH config file must be an absolute path, or empty for ~/.ssh/config.".into(),
                );
            }
            if p.forwards.is_empty() || p.forwards.len() > 50 {
                return Err("Add between 1 and 50 port forwards.".into());
            }
            for f in &p.forwards {
                if f.local_port == 0 || f.remote_port == 0 {
                    return Err("Ports must be between 1 and 65535.".into());
                }
                if !ports.insert(f.local_port) {
                    return Err(format!(
                        "Local port {} is used by more than one rule.",
                        f.local_port
                    ));
                }
                if f.remote_host.is_empty()
                    || f.remote_host.len() > 255
                    || !f
                        .remote_host
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "._-:".contains(c))
                {
                    return Err("Enter a destination hostname or IP address.".into());
                }
                if f.remote_host.contains(':')
                    && f.remote_host.parse::<std::net::Ipv6Addr>().is_err()
                {
                    return Err("Invalid IPv6 destination address.".into());
                }
                if f.label.len() > 120 {
                    return Err("Port labels must be at most 120 characters.".into());
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Default)]
pub struct Live {
    pub requested: bool,
    pub phase: String,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
    pub log: VecDeque<String>,
}

impl Live {
    fn record(&mut self, text: String) {
        self.log.push_back(text.chars().take(600).collect());
        while self.log.len() > 40 {
            self.log.pop_front();
        }
    }
    fn error(&mut self, error: String) {
        self.last_error = Some(error.clone());
        self.record(error);
    }
}

struct Runtime {
    stop: Arc<AtomicBool>,
    live: Arc<Mutex<Live>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Runtime {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct ManagerState {
    path: PathBuf,
    config: Config,
    load_error: Option<String>,
    runtimes: HashMap<String, Runtime>,
    shutting_down: bool,
}
pub struct SshState(Mutex<ManagerState>);

#[derive(Serialize)]
pub struct Snapshot {
    config: Config,
    states: HashMap<String, Live>,
    error: Option<String>,
}

impl SshState {
    pub fn load(path: PathBuf) -> Self {
        let result = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<Config>(&text)
                .map_err(|e| e.to_string())
                .and_then(|config| {
                    config.validate()?;
                    Ok(config)
                }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.to_string()),
        };
        let (config, load_error) = match result {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some(e)),
        };
        Self(Mutex::new(ManagerState {
            path,
            config,
            load_error,
            runtimes: HashMap::new(),
            shutting_down: false,
        }))
    }
    pub fn snapshot(&self) -> Snapshot {
        self.0.lock().unwrap().snapshot()
    }
    pub fn start_auto(&self) {
        let mut manager = self.0.lock().unwrap();
        let ids: Vec<_> = manager
            .config
            .profiles
            .iter()
            .filter(|p| p.auto_connect)
            .map(|p| p.id.clone())
            .collect();
        for id in ids {
            let _ = manager.start(&id);
        }
    }
    pub fn stop_all(&self) {
        let mut manager = self.0.lock().unwrap();
        manager.shutting_down = true;
        for runtime in manager.runtimes.values_mut() {
            runtime.stop();
        }
    }
}

impl ManagerState {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            config: self.config.clone(),
            states: self
                .runtimes
                .iter()
                .map(|(id, runtime)| (id.clone(), runtime.live.lock().unwrap().clone()))
                .collect(),
            error: self.load_error.clone(),
        }
    }
    fn start(&mut self, id: &str) -> Result<(), String> {
        if self.shutting_down {
            return Err("Routeway is exiting.".into());
        }
        let profile = self
            .config
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or("SSH profile not found.")?
            .clone();
        if self
            .runtimes
            .get(id)
            .is_some_and(|r| !r.stop.load(Ordering::SeqCst))
        {
            return Ok(());
        }
        let stop = Arc::new(AtomicBool::new(false));
        let live = Arc::new(Mutex::new(Live {
            requested: true,
            phase: "connecting".into(),
            ..Live::default()
        }));
        let worker_stop = stop.clone();
        let worker_live = live.clone();
        let worker = thread::Builder::new()
            .name(format!("ssh-{}", profile.id))
            .spawn(move || supervise(profile, worker_stop, worker_live))
            .map_err(|e| e.to_string())?;
        self.runtimes.insert(
            id.into(),
            Runtime {
                stop,
                live,
                worker: Some(worker),
            },
        );
        Ok(())
    }
    fn stop(&mut self, id: &str) {
        if let Some(runtime) = self.runtimes.get_mut(id) {
            runtime.stop();
        }
    }
    fn save(&mut self, config: Config) -> Result<(), String> {
        config.validate()?;
        if self.load_error.is_some() {
            return Err("Fix or back up the invalid ssh-tunnels.json before saving.".into());
        }
        fs::create_dir_all(self.path.parent().ok_or("Missing SSH settings directory")?)
            .map_err(|e| e.to_string())?;
        let temp = self.path.with_extension("json.tmp");
        fs::write(
            &temp,
            serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(temp, &self.path).map_err(|e| e.to_string())?;
        // Autoconnect preference alone must not interrupt an existing tunnel.
        let changed: Vec<_> = self
            .config
            .profiles
            .iter()
            .filter(|old| {
                !config.profiles.iter().any(|new| {
                    new.id == old.id
                        && new.host == old.host
                        && new.config_file == old.config_file
                        && new.forwards == old.forwards
                })
            })
            .map(|p| p.id.clone())
            .collect();
        for id in changed {
            self.stop(&id);
            self.runtimes.remove(&id);
        }
        self.config = config;
        Ok(())
    }
}

fn ssh_command(profile: &Profile) -> Command {
    #[cfg(windows)]
    let mut command = Command::new(
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("System32/OpenSSH/ssh.exe"),
    );
    #[cfg(not(windows))]
    let mut command = Command::new("/usr/bin/ssh");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.args([
        "-N",
        "-T",
        "-v",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "ExitOnForwardFailure=yes",
        "-o",
        "ConnectTimeout=15",
        "-o",
        "ConnectionAttempts=1",
        "-o",
        "ServerAliveInterval=20",
        "-o",
        "ServerAliveCountMax=3",
        "-o",
        "ControlMaster=no",
        "-o",
        "ControlPath=none",
        "-o",
        "ForkAfterAuthentication=no",
        "-o",
        "LogLevel=DEBUG1",
        "-o",
        "RemoteCommand=none",
        "-o",
        "PermitLocalCommand=no",
    ]);
    if !profile.config_file.is_empty() {
        command.arg("-F").arg(&profile.config_file);
    }
    for f in &profile.forwards {
        let host = if f.remote_host.contains(':') {
            format!("[{}]", f.remote_host)
        } else {
            f.remote_host.clone()
        };
        command.arg("-L").arg(format!(
            "127.0.0.1:{}:{}:{}",
            f.local_port, host, f.remote_port
        ));
    }
    command
        .arg(&profile.host)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    command
}

/// Own the entire process tree, including an optional ProxyJump helper.
struct ProcessTree {
    #[cfg(windows)]
    job: windows_sys::Win32::Foundation::HANDLE,
    #[cfg(unix)]
    group: i32,
}
impl ProcessTree {
    fn attach(child: &std::process::Child) -> Result<Self, String> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{Foundation::CloseHandle, System::JobObjects::*};
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let configured = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of_val(&info) as u32,
                );
                if configured == 0 || AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0
                {
                    let error = std::io::Error::last_os_error().to_string();
                    CloseHandle(job);
                    return Err(error);
                }
                Ok(Self { job })
            }
        }
        #[cfg(unix)]
        {
            Ok(Self {
                group: child.id() as i32,
            })
        }
    }
    fn kill(&mut self) {
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
        }
        #[cfg(unix)]
        unsafe {
            if self.group > 0 {
                libc::kill(-self.group, libc::SIGKILL);
                self.group = 0;
            }
        }
    }
}
impl Drop for ProcessTree {
    fn drop(&mut self) {
        self.kill();
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

fn wait_retry(stop: &AtomicBool) {
    for _ in 0..40 {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn supervise(profile: Profile, stop: Arc<AtomicBool>, live: Arc<Mutex<Live>>) {
    let mut first = true;
    while !stop.load(Ordering::SeqCst) {
        {
            let mut s = live.lock().unwrap();
            s.phase = if first { "connecting" } else { "reconnecting" }.into();
            s.pid = None;
            s.record("Connecting…".into());
        }
        first = false;
        // Refuse to claim an already occupied listener as our connection.
        let mut reservations = Vec::new();
        let occupied = profile.forwards.iter().find_map(|f| {
            match TcpListener::bind(("127.0.0.1", f.local_port)) {
                Ok(listener) => {
                    reservations.push(listener);
                    None
                }
                Err(_) => Some(f.local_port),
            }
        });
        drop(reservations);
        if let Some(port) = occupied {
            live.lock().unwrap().error(format!("Local port {port} is already in use. Stop the other tunnel or choose another port."));
            wait_retry(&stop);
            continue;
        }
        let mut child = match ssh_command(&profile).spawn() {
            Ok(child) => child,
            Err(e) => {
                live.lock()
                    .unwrap()
                    .error(format!("Cannot start OpenSSH: {e}"));
                wait_retry(&stop);
                continue;
            }
        };
        let mut tree = match ProcessTree::attach(&child) {
            Ok(tree) => tree,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                live.lock()
                    .unwrap()
                    .error(format!("Cannot supervise SSH process: {e}"));
                wait_retry(&stop);
                continue;
            }
        };
        live.lock().unwrap().pid = Some(child.id());
        let stderr = child.stderr.take().unwrap();
        let reader_live = live.clone();
        let expected: HashSet<_> = profile.forwards.iter().map(|f| f.local_port).collect();
        let reader = thread::spawn(move || {
            let mut authenticated = false;
            let mut listeners = HashSet::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if line.starts_with("Authenticated to ") {
                    authenticated = true;
                }
                if let Some(tail) = line
                    .split("Local forwarding listening on 127.0.0.1 port ")
                    .nth(1)
                {
                    if let Ok(port) = tail.trim_end_matches('.').parse::<u16>() {
                        listeners.insert(port);
                    }
                }
                let mut s = reader_live.lock().unwrap();
                if authenticated && expected.is_subset(&listeners) && s.phase != "connected" {
                    s.phase = "connected".into();
                    s.last_error = None;
                    s.record("SSH connected; all local forwards are listening.".into());
                }
                if !line.starts_with("debug")
                    && !line.starts_with("Authenticated to ")
                    && !line.starts_with("Transferred:")
                    && !line.starts_with("Bytes per second:")
                    && !line.trim().is_empty()
                {
                    s.error(line);
                }
            }
        });
        loop {
            if stop.load(Ordering::SeqCst) {
                tree.kill();
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            match child.try_wait() {
                Ok(Some(code)) => {
                    let mut s = live.lock().unwrap();
                    let message = format!("SSH exited ({code}). Retrying in 10 seconds.");
                    if s.last_error.is_none() {
                        s.error(message);
                    } else {
                        s.record(message);
                    }
                    break;
                }
                Ok(None) => thread::sleep(Duration::from_millis(250)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    live.lock().unwrap().error(e.to_string());
                    break;
                }
            }
        }
        tree.kill();
        let _ = reader.join();
        {
            let mut s = live.lock().unwrap();
            s.pid = None;
            s.phase = "reconnecting".into();
        }
        if !stop.load(Ordering::SeqCst) {
            wait_retry(&stop);
        }
    }
    let mut s = live.lock().unwrap();
    s.requested = false;
    s.phase = "stopped".into();
    s.pid = None;
    s.record("Stopped.".into());
}

#[tauri::command]
pub async fn get_ssh(app: AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<SshState>().snapshot())
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn save_ssh(app: AppHandle, config: Config) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SshState>();
        let mut manager = state.0.lock().unwrap();
        manager.save(config)?;
        Ok(manager.snapshot())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn control_ssh(app: AppHandle, id: String, action: String) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<SshState>();
        let mut manager = state.0.lock().unwrap();
        if !manager.config.profiles.iter().any(|p| p.id == id) {
            return Err("SSH profile not found.".into());
        }
        match action.as_str() {
            "start" => manager.start(&id)?,
            "stop" => manager.stop(&id),
            "restart" => {
                manager.stop(&id);
                manager.start(&id)?;
            }
            _ => return Err("Unknown SSH action.".into()),
        }
        Ok(manager.snapshot())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> Profile {
        Profile {
            id: "test".into(),
            name: "Test".into(),
            host: "user@example.org".into(),
            config_file: String::new(),
            auto_connect: false,
            forwards: vec![Forward {
                label: "DB".into(),
                local_port: 15432,
                remote_host: "localhost".into(),
                remote_port: 5432,
            }],
        }
    }
    #[test]
    fn rejects_options_and_shared_ports() {
        let mut config = Config {
            profiles: vec![profile()],
        };
        assert!(config.validate().is_ok());
        config.profiles[0].host = "-oProxyCommand=bad".into();
        assert!(config.validate().is_err());
        config.profiles[0] = profile();
        let mut other = profile();
        other.id = "other".into();
        config.profiles.push(other);
        assert!(config.validate().unwrap_err().contains("15432"));
    }
    #[test]
    fn destination_and_port_validation() {
        let mut p = profile();
        p.forwards[0].remote_host = "::1".into();
        assert!(Config {
            profiles: vec![p.clone()]
        }
        .validate()
        .is_ok());
        p.forwards[0].remote_host = "localhost:22:evil".into(); // must be a real IPv6 address when it contains colons
        assert!(Config {
            profiles: vec![p.clone()]
        }
        .validate()
        .is_err());
        p.forwards[0].local_port = 0;
        assert!(Config { profiles: vec![p] }.validate().is_err());
    }
    #[test]
    fn separate_arguments_and_loopback_only() {
        let mut p = profile();
        p.config_file = if cfg!(windows) {
            "C:\\SSH Folder\\config"
        } else {
            "/tmp/SSH Folder/config"
        }
        .into();
        let command = ssh_command(&p);
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&p.config_file));
        assert!(args.contains(&"127.0.0.1:15432:localhost:5432".into()));
        assert!(args.contains(&"StrictHostKeyChecking=yes".into()));
        assert_eq!(args.last().unwrap(), "user@example.org");
    }
    #[test]
    fn occupied_port_is_reported_and_stop_cancels_retry() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut p = profile();
        p.forwards[0].local_port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let live = Arc::new(Mutex::new(Live::default()));
        let s = stop.clone();
        let l = live.clone();
        let worker = thread::spawn(move || supervise(p, s, l));
        for _ in 0..50 {
            if live.lock().unwrap().last_error.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(live
            .lock()
            .unwrap()
            .last_error
            .as_ref()
            .unwrap()
            .contains("already in use"));
        stop.store(true, Ordering::SeqCst);
        worker.join().unwrap();
        assert_eq!(live.lock().unwrap().phase, "stopped");
    }

    #[test]
    #[ignore = "requires an explicitly supplied SSH config and reachable test host"]
    fn live_connection_restart_and_shutdown() {
        let config_file = PathBuf::from(
            std::env::var("ROUTEWAY_SSH_SMOKE_CONFIG").expect("Set ROUTEWAY_SSH_SMOKE_CONFIG"),
        );
        let state = SshState::load(config_file);
        assert!(state.snapshot().error.is_none());
        state.start_auto();
        let ids: Vec<_> = state
            .snapshot()
            .config
            .profiles
            .iter()
            .map(|p| p.id.clone())
            .collect();
        let await_connected = |state: &SshState| {
            for _ in 0..240 {
                let snapshot = state.snapshot();
                if ids.iter().all(|id| {
                    snapshot
                        .states
                        .get(id)
                        .is_some_and(|s| s.phase == "connected")
                }) {
                    return;
                }
                thread::sleep(Duration::from_millis(250));
            }
            for (id, s) in state.snapshot().states {
                eprintln!("{id}: {} {:?}", s.phase, s.log);
            }
            panic!("SSH did not connect");
        };
        await_connected(&state);
        // Simulate loss of each SSH child; the independent supervisors recover.
        let pids: Vec<_> = state
            .snapshot()
            .states
            .values()
            .filter_map(|s| s.pid)
            .collect();
        for pid in &pids {
            #[cfg(windows)]
            {
                let mut c = Command::new("taskkill.exe");
                use std::os::windows::process::CommandExt;
                c.creation_flags(0x08000000)
                    .args(["/F", "/PID", &pid.to_string()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                assert!(c.status().unwrap().success());
            }
            #[cfg(unix)]
            unsafe {
                assert_eq!(libc::kill(*pid as i32, libc::SIGKILL), 0);
            }
        }
        thread::sleep(Duration::from_secs(1));
        await_connected(&state);
        assert!(state
            .snapshot()
            .states
            .values()
            .all(|s| !pids.contains(&s.pid.unwrap())));
        // Metadata-only changes persist without replacing running children.
        let before: HashMap<_, _> = state
            .snapshot()
            .states
            .iter()
            .map(|(id, s)| (id.clone(), s.pid))
            .collect();
        {
            let mut manager = state.0.lock().unwrap();
            let mut config = manager.config.clone();
            for p in &mut config.profiles {
                p.auto_connect = false;
            }
            manager.save(config).unwrap();
        }
        assert!(state
            .snapshot()
            .states
            .iter()
            .all(|(id, s)| before[id] == s.pid));
        // Restart and stop, including releasing the listener ports.
        for id in &ids {
            let mut manager = state.0.lock().unwrap();
            manager.stop(id);
            manager.start(id).unwrap();
        }
        await_connected(&state);
        state.stop_all();
        for p in state.snapshot().config.profiles {
            for f in p.forwards {
                assert!(TcpListener::bind(("127.0.0.1", f.local_port)).is_ok());
            }
        }
    }
}
