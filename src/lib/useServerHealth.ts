import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getHealth, type Health } from "./ipc";

export type { Health, Sample } from "./ipc";

export const PROBE_WINDOW = 30;

export function useServerHealth(): Health {
  const [health, setHealth] = useState<Health>({ mode: null, samples: [] });

  useEffect(() => {
    let alive = true;
    void getHealth().then((current) => alive && setHealth(current));
    const unlisten = listen<Health>("health", (event) => setHealth(event.payload));
    return () => {
      alive = false;
      void unlisten.then((fn) => fn());
    };
  }, []);

  return health;
}

export function summarize(samples: Health["samples"]) {
  const answered = samples.filter((s) => s.ms !== null).map((s) => s.ms as number);
  const lossPct = samples.length
    ? Math.round(((samples.length - answered.length) / samples.length) * 100)
    : 0;
  const avg = answered.length
    ? Math.round(answered.reduce((sum, ms) => sum + ms, 0) / answered.length)
    : null;
  return { last: samples.at(-1) ?? null, avg, lossPct };
}
