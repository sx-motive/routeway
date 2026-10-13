import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { probeServer, type Probe } from "./ipc";

export const PROBE_INTERVAL_MS = 5000;
export const PROBE_WINDOW = 30;

export interface Sample {
  ms: number | null;
  error: string | null;
  at: number;
}

export interface Health {
  mode: Probe["mode"] | null;
  samples: Sample[];
}

async function windowIsShown(): Promise<boolean> {
  try {
    const win = getCurrentWindow();
    const [visible, minimized] = await Promise.all([win.isVisible(), win.isMinimized()]);
    return visible && !minimized;
  } catch {
    return true;
  }
}

export function useServerHealth(enabled: boolean, resetKey: string): Health {
  const [health, setHealth] = useState<Health>({ mode: null, samples: [] });

  useEffect(() => {
    setHealth({ mode: null, samples: [] });
    if (!enabled) return;

    let cancelled = false;
    let inFlight = false;

    const tick = async () => {
      if (inFlight || !(await windowIsShown())) return;
      inFlight = true;
      try {
        const probe = await probeServer();
        if (cancelled || !probe) return;
        setHealth((prev) => ({
          mode: probe.mode,
          samples: [
            ...prev.samples.slice(-(PROBE_WINDOW - 1)),
            { ms: probe.ms, error: probe.error, at: Date.now() },
          ],
        }));
      } catch {
        // a failed IPC call is not a server failure; the next tick retries
      } finally {
        inFlight = false;
      }
    };

    void tick();
    const timer = window.setInterval(() => void tick(), PROBE_INTERVAL_MS);
    const onFocus = () => void tick();
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [enabled, resetKey]);

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
