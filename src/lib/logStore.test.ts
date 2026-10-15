import { describe, expect, it, vi } from "vitest";
import type { LogEvent } from "./coreLog";
import { createLogStore } from "./logStore";

const event = (raw: string): LogEvent => ({
  raw, time: "12:00:00", level: "info", kind: "other",
  app: null, target: null, domain: null, route: null, reason: null, detail: null,
});

describe("log store", () => {
  it("keeps a stable snapshot until changed and collects while no view is subscribed", () => {
    const store = createLogStore();
    const initial = store.getSnapshot();
    expect(store.getSnapshot()).toBe(initial);
    store.append(event("first"));
    const notify = vi.fn();
    const unsubscribe = store.subscribe(notify);
    expect(store.getSnapshot().map((e) => e.raw)).toEqual(["first"]);
    const previous = store.getSnapshot();
    store.append(event("second"));
    expect(notify).toHaveBeenCalledTimes(1);
    expect(previous).toHaveLength(1);
    unsubscribe();
    store.append(event("third"));
    expect(notify).toHaveBeenCalledTimes(1);
    expect(store.getSnapshot()).toHaveLength(3);
  });

  it("bounds history to the most recent 300 events and clears it across remounts", () => {
    const store = createLogStore();
    store.reset(Array.from({ length: 310 }, (_, i) => event(String(i))));
    store.append(event("310"));
    expect(store.getSnapshot()).toHaveLength(300);
    expect(store.getSnapshot()[0].raw).toBe("11");
    expect(store.getSnapshot().at(-1)?.raw).toBe("310");
    store.clear();
    expect(store.getSnapshot()).toEqual([]);
    store.append(event("after clear"));
    expect(store.getSnapshot().map((e) => e.raw)).toEqual(["after clear"]);
  });
});
