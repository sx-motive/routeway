import { Effect, getCurrentWindow } from "@tauri-apps/api/window";

type Resolved = "dark" | "light";

// Acrylic blurs the windows behind the app; Mica only tints with the wallpaper.
const BACKDROP = Effect.Acrylic;

interface UADataWithHints {
  platform?: string;
  getHighEntropyValues?: (hints: string[]) => Promise<{ platformVersion?: string }>;
}

/**
 * Mica exists only on Windows 11. On anything else a transparent window would
 * show the desktop through the app, so glass stays off and the page paints an
 * opaque background. WebView2 reports Windows 11 as platformVersion 13+.
 */
async function supportsMica(): Promise<boolean> {
  if (!("__TAURI_INTERNALS__" in window)) return false;
  const uaData = (navigator as Navigator & { userAgentData?: UADataWithHints }).userAgentData;
  if (uaData?.platform !== "Windows" || !uaData.getHighEntropyValues) return false;
  try {
    const { platformVersion } = await uaData.getHighEntropyValues(["platformVersion"]);
    return Number(platformVersion?.split(".")[0]) >= 13;
  } catch {
    return false;
  }
}

let glass: Promise<boolean> | null = null;

/** Turns on Mica once per session; resolves to whether the window is glass. */
export function enableGlass(): Promise<boolean> {
  glass ??= supportsMica().then(async (ok) => {
    if (!ok) return false;
    const win = getCurrentWindow();
    try {
      // Shadow on an undecorated Windows 11 window also brings rounded corners.
      await win.setShadow(true);
      await win.setEffects({ effects: [BACKDROP] });
      document.documentElement.dataset.glass = "on";
      return true;
    } catch (err) {
      console.warn("[glass] unavailable", err);
      return false;
    }
  });
  return glass;
}

/** Mica follows the window theme, so keep it in step with the app theme. */
export async function syncWindowTheme(theme: Resolved | null) {
  if (!("__TAURI_INTERNALS__" in window)) return;
  try {
    const win = getCurrentWindow();
    await win.setTheme(theme);
    if (await enableGlass()) await win.setEffects({ effects: [BACKDROP] });
  } catch (err) {
    console.warn("[glass] theme sync failed", err);
  }
}
