import { invoke } from "@tauri-apps/api/core";
import type { Server } from "./vless";

export interface Settings {
  server: Server | null;
  domains: string[];
  processes: string[];
  listen_port: number;
  system_proxy: boolean;
  autostart: boolean;
}

export interface Status {
  settings: Settings;
  running: boolean;
  system_proxy_active: boolean;
  log: string[];
}

export const getStatus = () => invoke<Status>("get_status");

export const saveSettings = (settings: Settings) =>
  invoke<Status>("save_settings", { settings });

export const connect = () => invoke<Status>("connect");

export const disconnect = () => invoke<Status>("disconnect");

export const checkIp = (throughServer: boolean) =>
  invoke<string>("check_ip", { throughServer });

export const revealConfig = () => invoke<void>("reveal_config");
