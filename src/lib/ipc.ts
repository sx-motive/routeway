import { invoke } from "@tauri-apps/api/core";
import type { Server } from "./vless";
import type { LogEvent } from "./coreLog";

export interface Settings {
  server: Server | null;
  domains: string[];
  processes: string[];
  direct: string[];
  listen_port: number;
  system_proxy: boolean;
  tun: boolean;
  language: string;
  autostart: boolean;
  auto_connect: boolean;
}

export interface Status {
  settings: Settings;
  running: boolean;
  connection_requested?: boolean;
  system_proxy_active: boolean;
  tun_active: boolean;
  log: LogEvent[];
}

export const getStatus = () => invoke<Status>("get_status");

export const initialize = () =>
  invoke<{ status: Status; error: string | null }>("initialize");

export const saveSettings = (settings: Settings) =>
  invoke<Status>("save_settings", { settings });

export const setLanguage = (language: string) =>
  invoke<Status>("set_language", { language });

export const getRules = () => invoke<string>("get_rules");

export const revealRules = () => invoke<void>("reveal_rules");

export const saveRules = (text: string) => invoke<Status>("save_rules", { text });

export const connect = () => invoke<Status>("connect");

export const disconnect = () => invoke<Status>("disconnect");

export type MaintenanceAction = "restart" | "clean" | "app";

export interface Diagnostics {
  processes: { pid: number; parent_pid: number; name: string; role: "app" | "core" | "host" | "webview" | "child" }[];
  core_running: boolean;
  system_proxy_active: boolean;
  tun_active: boolean;
  listen_port: number;
}

export const getDiagnostics = () => invoke<Diagnostics>("get_diagnostics");
export const restartCore = () => invoke<Status>("restart_core");
export const cleanCore = () => invoke<Status>("clean_core");
export const restartApp = () => invoke<void>("restart_app");

export const checkIp = (throughServer: boolean) =>
  invoke<string>("check_ip", { throughServer });

export interface Sample {
  ms: number | null;
  error: string | null;
  at: number;
}

export interface Health {
  tcp: Sample[];
  tunnel: Sample[];
  tcp_available: boolean;
  tunnel_active: boolean;
  recovery?: "idle" | "healthy" | "checking" | "recovering" | "waiting_network" | "unavailable";
}

export const getHealth = () => invoke<Health>("get_health");

export const revealConfig = () => invoke<void>("reveal_config");

export const openExternal = (url: string) =>
  invoke<void>("plugin:opener|open_url", { url });

// Only these two window operations are needed; keep the full Window API out of the bundle.
export const minimizeWindow = () => invoke<void>("minimize_window");
export const hideWindow = () => invoke<void>("hide_window");
