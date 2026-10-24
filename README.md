# Routeway

Desktop traffic routing for **Windows and macOS**. Send selected websites and apps through your own VLESS/REALITY server while keeping other traffic direct. Built with Tauri, React and sing-box.

## Features

- Domain and process rules, with always-direct exceptions.
- System proxy and TUN modes; TUN supports TCP/UDP and requires administrator privileges.
- Separate TCP and tunnel HTTPS health metrics, with automatic recovery and restart limits.
- Process diagnostics, core restart and cleanup controls.
- Tray operation, optional launch at login and auto-connect, signed automatic updates.

## Get started

Download a build from [Releases](https://github.com/sx-motive/routeway/releases), add your server, choose domains or apps, and connect. Installers are unsigned; update artifacts are signed.

System proxy mode handles traffic from apps that use the proxy. Use TUN for apps that ignore it or need UDP. Closing the window keeps routing active; use **Quit** in the tray to exit.

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
