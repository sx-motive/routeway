import { invoke } from "@tauri-apps/api/core";
import type { Server } from "./vless";

export interface Settings {
  server: Server | null;
  domains: string[];
  processes: string[];
  listen_port: number;
  system_proxy: boolean;
  tun: boolean;
  language: string;
  autostart: boolean;
}

export interface Status {
  settings: Settings;
  running: boolean;
  system_proxy_active: boolean;
  tun_active: boolean;
  log: string[];
}

export const getStatus = () => invoke<Status>("get_status");

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

export interface Probe {
  mode: "tcp" | "tunnel";
  ms: number | null;
  error: string | null;
}

export const probeServer = () => invoke<Probe | null>("probe_server");

export const revealConfig = () => invoke<void>("reveal_config");
