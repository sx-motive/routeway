import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getHealth, type Health, type Sample } from "./ipc";

export type { Health, Sample } from "./ipc";

export const PROBE_WINDOW = 30;

export function useServerHealth(): Health {
  const [health, setHealth] = useState<Health>({ tcp: [], tunnel: [], tcp_available: true, tunnel_active: false });

  useEffect(() => {
    let alive = true;
    let receivedEvent = false;
    const unlisten = listen<Health>("health", (event) => {
      receivedEvent = true;
      if (alive) setHealth(event.payload);
    });
    // Subscribe before taking a snapshot; a late snapshot must not overwrite a newer event.
    void unlisten.then(() => getHealth()).then((current) => {
      if (alive && !receivedEvent) setHealth(current);
    }).catch((err) => console.warn("[health] snapshot failed", err));
    return () => {
      alive = false;
      void unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);

  return health;
}

export function summarize(samples: Sample[]) {
  const answered = samples.filter((s) => s.ms !== null).map((s) => s.ms as number);
  const lossPct = samples.length
    ? Math.round(((samples.length - answered.length) / samples.length) * 100)
    : 0;
  const avg = answered.length
    ? Math.round(answered.reduce((sum, ms) => sum + ms, 0) / answered.length)
    : null;
  return { last: samples.at(-1) ?? null, avg, lossPct };
}
