import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdateAvailable {
  version: string;
  notes?: string;
  install: () => Promise<void>;
}

let started = false;

export async function checkForUpdate(): Promise<UpdateAvailable | null> {
  if (started) return null;
  started = true;

  try {
    const update = await check();
    if (!update) return null;
    return wrap(update);
  } catch (err) {
    console.warn("[updater] check failed", err);
    return null;
  }
}

function wrap(update: Update): UpdateAvailable {
  return {
    version: update.version,
    notes: update.body || undefined,
    install: async () => {
      await update.downloadAndInstall();
      await relaunch();
    },
  };
}
