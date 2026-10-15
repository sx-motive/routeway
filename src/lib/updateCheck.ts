import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdateAvailable {
  version: string;
  notes?: string;
  install: () => Promise<void>;
}

let pending: Promise<UpdateAvailable | null> | undefined;

export function checkForUpdate(): Promise<UpdateAvailable | null> {
  return pending ??= findUpdate();
}

async function findUpdate(): Promise<UpdateAvailable | null> {
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
