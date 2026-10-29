import { invoke } from "@tauri-apps/api/core";

export interface SshForward {
  label: string;
  local_port: number;
  remote_host: string;
  remote_port: number;
}
export interface SshProfile {
  id: string;
  name: string;
  host: string;
  config_file: string;
  auto_connect: boolean;
  forwards: SshForward[];
}
export interface SshLive {
  requested: boolean;
  phase: "connecting" | "connected" | "reconnecting" | "stopped";
  pid: number | null;
  last_error: string | null;
  log: string[];
}
export interface SshSnapshot {
  config: { profiles: SshProfile[] };
  states: Record<string, SshLive>;
  error: string | null;
}
export const getSsh = () => invoke<SshSnapshot>("get_ssh");
export const saveSsh = (profiles: SshProfile[]) => invoke<SshSnapshot>("save_ssh", { config: { profiles } });
export const controlSsh = (id: string, action: "start" | "stop" | "restart") => invoke<SshSnapshot>("control_ssh", { id, action });

export const revealSshConfig = (configFile: string) => invoke<void>("reveal_ssh_config", { configFile });
