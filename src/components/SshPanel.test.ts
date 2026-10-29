// @vitest-environment jsdom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { SshSnapshot } from "../lib/ssh";
const ipc = vi.hoisted(() => ({ getSsh: vi.fn(), saveSsh: vi.fn(), controlSsh: vi.fn() }));
vi.mock("../lib/ssh", () => ipc);
import SshPanel from "./SshPanel";

const connected: SshSnapshot = {
  config: { profiles: [{ id: "web", name: "Web", host: "web1", config_file: "", auto_connect: true, forwards: [{ label: "API", local_port: 19708, remote_host: "localhost", remote_port: 7008 }] }] },
  states: { web: { phase: "connected", requested: true, pid: 123, last_error: null, log: ["SSH connected"] } }, error: null,
};
const stopped: SshSnapshot = { ...connected, states: { web: { ...connected.states.web, phase: "stopped", requested: false, pid: null } } };
let root: Root;
let container: HTMLDivElement;
beforeEach(() => {
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  vi.resetAllMocks();
  ipc.getSsh.mockResolvedValue(connected);
  ipc.controlSsh.mockResolvedValue(stopped);
  ipc.saveSsh.mockResolvedValue(connected);
  container = document.createElement("div"); document.body.append(container); root = createRoot(container);
});
afterEach(async () => { await act(async () => root.unmount()); container.remove(); vi.useRealTimers(); vi.unstubAllGlobals(); });
const mount = async () => { await act(async () => root.render(createElement(SshPanel))); };
const button = (label: string) => {
  const found = Array.from(container.querySelectorAll("button")).find((b) => (b.getAttribute("aria-label") ?? b.textContent?.trim()) === label);
  expect(found, label).toBeTruthy(); return found!;
};

it("shows local ports and stops only the selected SSH profile", async () => {
  await mount();
  expect(container.textContent).toContain("Connected"); expect(container.textContent).not.toContain("19708");
  await act(async () => button("Connect Web").click());
  expect(ipc.controlSsh).toHaveBeenCalledWith("web", "stop");
  expect(container.textContent).toContain("Stopped");
});

it("persists autoconnect independently without invoking a connection action", async () => {
  await mount();
  await act(async () => button("Edit: Web").click());
  await act(async () => container.querySelector<HTMLButtonElement>('[role="switch"]')!.click());
  await act(async () => container.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
  expect(ipc.saveSsh).toHaveBeenCalledWith([{ ...connected.config.profiles[0], auto_connect: false }]);
  expect(ipc.controlSsh).not.toHaveBeenCalled();
});

it("keeps connection errors visible and never claims a failed tunnel is connected", async () => {
  ipc.getSsh.mockResolvedValue({ ...connected, states: { web: { ...connected.states.web, phase: "reconnecting", last_error: "Local port 19708 is already in use" } } });
  await mount();
  expect(container.querySelector('[role="status"]')?.textContent).toContain("Reconnecting");
  await act(async () => button("Edit: Web").click());
  expect(container.textContent).toContain("already in use");
});

it("does not overwrite an explicit stop with an older polling result", async () => {
  vi.useFakeTimers();
  await mount();
  let resolve!: (snapshot: SshSnapshot) => void;
  ipc.getSsh.mockReturnValueOnce(new Promise<SshSnapshot>((done) => { resolve = done; }));
  await act(async () => { await vi.advanceTimersByTimeAsync(1500); });
  await act(async () => button("Connect Web").click());
  await act(async () => resolve(connected));
  expect(container.querySelector('[role="status"]')?.textContent).toContain("Stopped");
});

it("leaves the saved profiles intact after a failed edit", async () => {
  ipc.saveSsh.mockRejectedValue(new Error("Local port is duplicated"));
  await mount();
  await act(async () => button("Edit: Web").click());
  await act(async () => container.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
  expect(container.querySelector('[role="alert"]')?.textContent).toContain("duplicated");
  expect(container.querySelector("form")).not.toBeNull();
  expect(container.querySelector<HTMLInputElement>('input[inputmode="numeric"]')?.value).toBe("19708");
});

it("requires an inline confirmation before deleting an active profile", async () => {
  ipc.saveSsh.mockResolvedValue({ config: { profiles: [] }, states: {}, error: null });
  await mount();
  await act(async () => button("Edit: Web").click());
  await act(async () => button("Delete profile").click());
  expect(ipc.saveSsh).not.toHaveBeenCalled();
  expect(container.textContent).toContain("Delete Web and stop its tunnels?");
  await act(async () => button("Delete profile").click());
  expect(ipc.saveSsh).toHaveBeenCalledWith([]);
  expect(container.textContent).toContain("Add an SSH server");
});

it("returns from the editor without saving", async () => {
  await mount();
  await act(async () => button("Edit: Web").click());
  expect(container.querySelector("form")).not.toBeNull();
  await act(async () => button("Back").click());
  expect(container.querySelector("form")).toBeNull();
  expect(button("Connect Web")).toBeTruthy();
  expect(ipc.saveSsh).not.toHaveBeenCalled();
});
