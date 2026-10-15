import { beforeEach, expect, it, vi } from "vitest";

const { check, relaunch } = vi.hoisted(() => ({ check: vi.fn(), relaunch: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch }));

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
});

it("shares the in-flight update across remounts and preserves installation", async () => {
  const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
  check.mockResolvedValue({ version: "1.0.0", body: "notes", downloadAndInstall });
  const { checkForUpdate } = await import("./updateCheck");
  const first = checkForUpdate();
  expect(checkForUpdate()).toBe(first);
  const update = await first;
  expect(check).toHaveBeenCalledTimes(1);
  expect(update?.version).toBe("1.0.0");
  await update?.install();
  expect(downloadAndInstall).toHaveBeenCalledTimes(1);
  expect(relaunch).toHaveBeenCalledTimes(1);
  expect(downloadAndInstall.mock.invocationCallOrder[0]).toBeLessThan(relaunch.mock.invocationCallOrder[0]);
});
