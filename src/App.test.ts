// @vitest-environment jsdom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { LogEvent } from "./lib/coreLog";
import type { Health, Status } from "./lib/ipc";

const { handlers, ipc } = vi.hoisted(() => ({
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
  ipc: {
    initialize: vi.fn(), getHealth: vi.fn(), getStatus: vi.fn(),
    checkIp: vi.fn(), getRules: vi.fn(), saveRules: vi.fn(),
    saveSettings: vi.fn(), setLanguage: vi.fn(), connect: vi.fn(), disconnect: vi.fn(),
    revealRules: vi.fn(), revealConfig: vi.fn(), minimizeWindow: vi.fn(), hideWindow: vi.fn(),
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
  ipc.getHealth.mockResolvedValue({ mode: null, samples: [] });
  ipc.getStatus.mockResolvedValue(status);
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
  const button = Array.from(document.querySelectorAll("button")).find((button) => button.textContent?.trim() === text);
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
    mode: "tcp", samples: [{ ms: 42, error: null, at: 1000 }],
  } }));
  expect(container.textContent).toContain(translate("en", "health.ms", { ms: 42 }));
  await act(async () => resolve({ mode: null, samples: [] }));
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
