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

export const checkIp = (throughServer: boolean) =>
  invoke<string>("check_ip", { throughServer });

export interface Sample {
  ms: number | null;
  error: string | null;
  at: number;
}

export interface Health {
  mode: "tcp" | "tunnel" | null;
  samples: Sample[];
}

export const getHealth = () => invoke<Health>("get_health");

export const revealConfig = () => invoke<void>("reveal_config");

// Only these two window operations are needed; keep the full Window API out of the bundle.
export const minimizeWindow = () => invoke<void>("minimize_window");
export const hideWindow = () => invoke<void>("hide_window");
