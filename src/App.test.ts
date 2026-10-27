// @vitest-environment jsdom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { LogEvent } from "./lib/coreLog";
import type { Health, Status } from "./lib/ipc";

const { handlers, ipc } = vi.hoisted(() => ({
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
  ipc: {
    initialize: vi.fn(), getHealth: vi.fn(), checkHealth: vi.fn(), getStatus: vi.fn(),
    checkIp: vi.fn(), getRules: vi.fn(), saveRules: vi.fn(),
    saveSettings: vi.fn(), setLanguage: vi.fn(), connect: vi.fn(), disconnect: vi.fn(),
    revealRules: vi.fn(), revealConfig: vi.fn(), minimizeWindow: vi.fn(), hideWindow: vi.fn(),
    getDiagnostics: vi.fn(), restartCore: vi.fn(), cleanCore: vi.fn(), restartApp: vi.fn(),
    openExternal: vi.fn(),
  },
}));
vi.mock("./lib/ipc", () => ipc);
vi.mock("./lib/updateCheck", () => ({ checkForUpdate: async () => null }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    handlers.set(name, callback);
    return () => { if (handlers.get(name) === callback) handlers.delete(name); };
  }),
}));

import { App } from "./App";
import { translate } from "./i18n";

let root: Root;
let container: HTMLDivElement;
const status: Status = {
  running: true, system_proxy_active: true, tun_active: false, log: [],
  settings: {
    server: { label: "Test", address: "example.org", port: 443, uuid: "test", flow: "", sni: "", public_key: "", short_id: "", fingerprint: "chrome" },
    domains: [], processes: [], direct: [], listen_port: 2080,
    system_proxy: true, tun: false, language: "en", autostart: false, auto_connect: false,
  },
};

beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  handlers.clear();
  vi.resetAllMocks();
  ipc.initialize.mockResolvedValue({ status, error: null });
  ipc.getHealth.mockResolvedValue({ tunnel: [], tunnel_active: false });
  ipc.getStatus.mockResolvedValue(status);
  ipc.openExternal.mockResolvedValue(undefined);
  ipc.getDiagnostics.mockResolvedValue({
    processes: [{ pid: 123, parent_pid: 1, name: "routeway.exe", role: "app" }],
    core_running: true, system_proxy_active: true, tun_active: false, listen_port: 2080,
  });
  ipc.getRules.mockResolvedValue('{"apps":[],"domains":[],"direct":[]}');
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  expect(handlers.size).toBe(0);
  container.remove();
});

async function mount() {
  await act(async () => root.render(createElement(App)));
}

async function click(text: string) {
  const button = Array.from(document.querySelectorAll("button")).find((button) => (button.getAttribute("aria-label") ?? button.textContent?.trim()) === text);
  expect(button, `button ${text}`).toBeTruthy();
  await act(async () => button!.click());
  if (text === "Settings") {
    await vi.waitFor(async () => {
      await act(async () => {});
      expect(container.textContent).toContain(translate("en", "settings.general"));
    });
  }
}

it("collects logs on Settings and keeps a cleared log empty after switching tabs", async () => {
  await mount();
  await click("Settings");
  const event: LogEvent = {
    time: "12:00:00", level: "info", kind: "other", raw: "background event",
    app: null, target: null, domain: null, route: null, reason: null, detail: null,
  };
  await act(async () => handlers.get("core-log")!({ payload: event }));
  await click(translate("en", "tab.home"));
  expect(container.textContent).toContain("background event");
  await click("Clear");
  await click("Settings");
  await click(translate("en", "tab.home"));
  expect(container.textContent).not.toContain("background event");
});

it("does not overwrite a live health event with an older snapshot", async () => {
  let resolve!: (health: Health) => void;
  ipc.getHealth.mockReturnValue(new Promise<Health>((done) => { resolve = done; }));
  await mount();
  await act(async () => handlers.get("health")!({ payload: {
    tunnel: [{ ms: 42, error: null, at: 1000 }], tunnel_active: true,
  } }));
  expect(container.textContent).toContain(translate("en", "health.ms", { ms: 42 }));
  await act(async () => resolve({ tunnel: [], tunnel_active: false }));
  expect(container.textContent).toContain(translate("en", "health.ms", { ms: 42 }));
});

it("starts both IP checks before either one completes", async () => {
  const resolve = new Map<boolean, (ip: string) => void>();
  ipc.checkIp.mockImplementation((server: boolean) => new Promise<string>((done) => resolve.set(server, done)));
  await mount();
  await click("Settings");
  const checkButton = Array.from(container.querySelectorAll("button")).find((button) => button.textContent?.trim() === translate("en", "check.run"));
  expect(checkButton).toBeTruthy();
  await act(async () => checkButton!.click());
  expect(ipc.checkIp.mock.calls).toEqual([[false], [true]]);
  await act(async () => resolve.get(true)!("2.2.2.2"));
  await act(async () => resolve.get(false)!("1.1.1.1"));
  expect(container.textContent).toContain("1.1.1.1");
  expect(container.textContent).toContain("2.2.2.2");
});

it("opens the lazy rules dialog and reports load errors without saving unloaded text", async () => {
  ipc.getRules.mockRejectedValue(new Error("Cannot read rules"));
  await mount();
  await click("Settings");
  const edit = Array.from(container.querySelectorAll("button")).find((button) => button.title === translate("en", "rules.edit"));
  expect(edit).toBeTruthy();
  await act(async () => edit!.click());
  // React.lazy resolves a separate module on first use.
  await vi.waitFor(async () => {
    await act(async () => {});
    expect(document.querySelector('[role="dialog"]')).toBeTruthy();
  });
  expect(document.body.textContent).toContain("Cannot read rules");
  const area = document.querySelector("textarea")!;
  await act(async () => area.dispatchEvent(new KeyboardEvent("keydown", { key: "s", ctrlKey: true, bubbles: true })));
  expect(ipc.saveRules).not.toHaveBeenCalled();
  await click("Cancel");
  expect(document.querySelector('[role="dialog"]')).toBeNull();
});

it("restarts the core once and refreshes the process list after completion", async () => {
  let finish!: (next: Status) => void;
  ipc.restartCore.mockReturnValue(new Promise<Status>((resolve) => { finish = resolve; }));
  await mount();
  await click("Settings");
  expect(container.textContent).toContain("routeway.exe");
  await click(translate("en", "debug.restartCore"));
  const restart = Array.from(container.querySelectorAll("button"))
    .find((button) => button.getAttribute("aria-label") === translate("en", "debug.restartCore"))!;
  expect(restart.disabled).toBe(true);
  await act(async () => restart.click());
  expect(ipc.restartCore).toHaveBeenCalledTimes(1);
  ipc.getDiagnostics.mockResolvedValue({
    processes: [{ pid: 456, parent_pid: 123, name: "sing-box.exe", role: "core" }],
    core_running: true, system_proxy_active: true, tun_active: false, listen_port: 2080,
  });
  await act(async () => finish(status));
  expect(container.textContent).toContain("sing-box.exe");
  expect(container.textContent).toContain("456");
  expect(restart.disabled).toBe(false);
});

it("cleanup updates connection state without restarting the core", async () => {
  ipc.cleanCore.mockResolvedValue({ ...status, running: false, system_proxy_active: false });
  await mount();
  await click("Settings");
  await click(translate("en", "debug.clean"));
  expect(ipc.cleanCore).toHaveBeenCalledOnce();
  expect(ipc.restartCore).not.toHaveBeenCalled();
  await click(translate("en", "tab.home"));
  expect(container.textContent).toContain(translate("en", "status.off"));
});

it("shows a failed maintenance action and keeps the controls usable", async () => {
  ipc.restartCore.mockRejectedValue(new Error("Server check failed"));
  await mount();
  await click("Settings");
  await click(translate("en", "debug.restartCore"));
  expect(container.textContent).toContain("Server check failed");
  const cleanup = Array.from(container.querySelectorAll("button"))
    .find((button) => button.getAttribute("aria-label") === translate("en", "debug.clean"))!;
  expect(cleanup.disabled).toBe(false);
});

it("allows manual disconnection while the core is down and recovery is pending", async () => {
  ipc.initialize.mockResolvedValue({
    status: { ...status, running: false, connection_requested: true }, error: null,
  });
  ipc.disconnect.mockResolvedValue({ ...status, running: false, connection_requested: false });
  await mount();
  const toggle = container.querySelector<HTMLButtonElement>('[role="switch"]')!;
  expect(toggle.getAttribute("aria-checked")).toBe("true");
  await act(async () => toggle.click());
  expect(ipc.disconnect).toHaveBeenCalledOnce();
  expect(ipc.connect).not.toHaveBeenCalled();
  expect(toggle.getAttribute("aria-checked")).toBe("false");
});

it("refreshes connection state after background recovery", async () => {
  await mount();
  ipc.getStatus.mockResolvedValue({ ...status, running: false, connection_requested: true });
  await act(async () => handlers.get("connection-changed")!({ payload: null }));
  expect(container.textContent).toContain(translate("en", "recovery.recovering"));
});

it("shows tunnel latency and its failure rate", async () => {
  ipc.getHealth.mockResolvedValue({
    tunnel: [{ ms: null, error: "timeout", at: 1000 }],
    tunnel_active: true,
  });
  await mount();
  expect(container.textContent).toContain(translate("en", "health.modeTunnel"));
  expect(container.textContent).toContain(translate("en", "health.failures", { pct: 100 }));
  await act(async () => handlers.get("health")!({ payload: {
    tunnel: [{ ms: null, error: "timeout", at: 1000 }, { ms: 260, error: null, at: 2000 }],
    tunnel_active: true,
  } }));
  expect(container.textContent).toContain(translate("en", "health.ms", { ms: 260 }));
  expect(container.textContent).toContain(translate("en", "health.failures", { pct: 50 }));
});

it("checks the tunnel on demand", async () => {
  ipc.getHealth.mockResolvedValue({ tunnel: [], tunnel_active: true });
  ipc.checkHealth.mockResolvedValue({ tunnel: [], tunnel_active: true });
  await mount();
  const button = container.querySelector<HTMLButtonElement>(
    `button[aria-label="${translate("en", "health.checkNow")}"]`,
  )!;
  await act(async () => button.click());
  expect(ipc.checkHealth).toHaveBeenCalledOnce();
});

it("does not display old tunnel latency after disconnection", async () => {
  ipc.getHealth.mockResolvedValue({
    tunnel: [{ ms: 260, error: null, at: 1000 }],
    tunnel_active: false,
  });
  await mount();
  expect(container.textContent).not.toContain(translate("en", "health.ms", { ms: 260 }));
  expect(container.textContent).toContain(translate("en", "health.afterConnect"));
  expect(container.querySelector(`button[aria-label="${translate("en", "health.checkNow")}"]`)).toBeNull();
});

it("opens project and license links through the native opener", async () => {
  await mount();
  await click("Settings");
  for (const url of ["https://github.com/sx-motive/routeway", "https://www.gnu.org/licenses/gpl-3.0.html"]) {
    const link = container.querySelector<HTMLAnchorElement>(`a[href="${url}"]`)!;
    await act(async () => link.click());
    expect(ipc.openExternal).toHaveBeenLastCalledWith(url);
  }
  expect(ipc.openExternal).toHaveBeenCalledTimes(2);
});

it("reports a native link-opening error in settings", async () => {
  ipc.openExternal.mockRejectedValue(new Error("No default browser"));
  await mount();
  await click("Settings");
  const link = container.querySelector<HTMLAnchorElement>('a[href="https://github.com/sx-motive/routeway"]')!;
  await act(async () => link.click());
  expect(container.querySelector('[role="alert"]')?.textContent).toContain("No default browser");
});
