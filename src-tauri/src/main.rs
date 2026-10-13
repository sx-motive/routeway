// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some(routeway_lib::TUN_HOST_FLAG) {
        let dir = args.get(2).map(std::path::PathBuf::from);
        let parent = args.get(3).and_then(|v| v.parse::<u32>().ok());
        let code = match (dir, parent) {
            (Some(dir), Some(parent)) => routeway_lib::run_tun_host(dir, parent),
            _ => 2,
        };
        std::process::exit(code);
    }
    routeway_lib::run()
}
