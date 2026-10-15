import { stampLocalTime, type LogEvent } from "./coreLog";

const LIMIT = 300;

// Keep collecting while the log tab is unmounted, without rerendering App.
export function createLogStore() {
  let events: LogEvent[] = [];
  const listeners = new Set<() => void>();
  const replace = (next: LogEvent[]) => {
    events = next;
    listeners.forEach((listener) => listener());
  };
  return {
    getSnapshot: () => events,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    reset: (initial: LogEvent[]) => replace(initial.slice(-LIMIT).map(stampLocalTime)),
    append: (event: LogEvent) => replace([...events.slice(1 - LIMIT), stampLocalTime(event)]),
    clear: () => replace([]),
  };
}

export type LogStore = ReturnType<typeof createLogStore>;
