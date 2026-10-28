# Routeway

Desktop traffic routing for **Windows and macOS**. Send selected websites and apps through your own VLESS/REALITY server while keeping other traffic direct. Built with Tauri, React and sing-box.

## Features

- Domain and process rules, with always-direct exceptions.
- System proxy and TUN modes; TUN supports TCP/UDP and requires administrator privileges.
- A light tunnel check once a minute, with a manual "check now" button; the core restarts automatically only if its process exits.
- Process diagnostics, core restart and cleanup controls.
- Tray operation, optional launch at login and auto-connect, signed automatic updates.
- SSH port forwards with independent connection controls, status, logs and automatic reconnection on Windows and macOS.

## Get started

Download a build from [Releases](https://github.com/sx-motive/routeway/releases), add your server, choose domains or apps, and connect. Installers are unsigned; update artifacts are signed.

System proxy mode handles traffic from apps that use the proxy. Use TUN for apps that ignore it or need UDP. Closing the window keeps routing active; use **Quit** in the tray to exit.

## SSH tunnels

The SSH tab uses the system OpenSSH client (Windows OpenSSH Client or `/usr/bin/ssh` on macOS). Configure key authentication and verify the host fingerprint in a terminal first. Use a host alias from `~/.ssh/config` or `user@hostname`; an optional absolute SSH config path is supported. Encrypted keys can use an existing SSH agent. Routeway never stores private keys or passwords.

Each profile forwards one or more ports on `127.0.0.1` to services reachable from the SSH server. “Connected” means SSH authenticated and all configured local listeners opened; it does not check the remote applications. Occupied ports and connection failures appear in the profile, with a retry every ten seconds. OpenSSH keepalives detect broken connections.

SSH profiles are stored separately in `ssh-tunnels.json` in the Routeway configuration directory. Autoconnect runs when Routeway starts; enable launch at login in Settings to also start after signing in. Proxy disconnect, cleanup and core restart do not stop SSH. Closing the window leaves tunnels in the tray; fully quitting or restarting Routeway stops them. Changes to forwarding rules stop the affected profile; reconnect to use the new settings. Stop other tunnel managers before connecting profiles that use the same local ports.

Launching Routeway with `--connect` also connects routing for that launch without changing the saved auto-connect preference.

## Development

Requires Node.js, stable Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
npm ci
npm run tauri:dev
```

The dev command downloads the pinned sing-box core automatically. For a fresh desktop build, run `npm run core` first.

```sh
npm run tauri:build                              # Desktop packages
npm run build                                    # Frontend build and typecheck
npm test                                         # Frontend tests
cargo test --manifest-path src-tauri/Cargo.toml    # Rust tests
```

Core version: `CORE_VERSION` in `scripts/fetch-core.mjs`. Regenerate platform icons and theme-aware UI logos with `node scripts/make-icon.mjs`.

## License

[GPL-3.0-or-later](LICENSE), without warranty. Copyright (C) 2026 Routeway contributors.

Bundled sing-box retains its [upstream license](licenses/sing-box-LICENSE.txt). See [NOTICE](NOTICE) for its exact version, source code and build instructions.
