export interface LogEvent {
  time: string;
  level: "info" | "warn" | "error";
  kind: "started" | "stopped" | "listening" | "interface" | "connection_failed" | "other";
  app: string | null;
  target: string | null;
  domain: string | null;
  route: "direct" | "proxy" | null;
  reason: string | null;
  detail: string | null;
  raw: string;
}

export type ReasonKey = "timeout" | "refused" | "reset" | "unreachable" | "notFound" | "eof";

export interface LogRow {
  key: string;
  event: LogEvent;
  count: number;
  ports: string[];
  lastTime: string;
}

export function splitTarget(target: string | null): { host: string | null; port: string | null } {
  if (!target) return { host: null, port: null };
  const at = target.lastIndexOf(":");
  if (at <= 0 || target.indexOf(":") !== at) return { host: target.replace(/^\[|\]$/g, ""), port: null };
  return { host: target.slice(0, at), port: target.slice(at + 1) };
}

export function reasonKey(reason: string | null): ReasonKey | null {
  if (!reason) return null;
  const text = reason.toLowerCase();
  if (/timeout|timed out|deadline/.test(text)) return "timeout";
  if (/refused/.test(text)) return "refused";
  if (/reset|forcibly closed|broken pipe/.test(text)) return "reset";
  if (/unreachable|no route/.test(text)) return "unreachable";
  if (/no such host|nxdomain|not found/.test(text)) return "notFound";
  if (/\beof\b/.test(text)) return "eof";
  return null;
}

function rowKey(event: LogEvent): string {
  const { host } = splitTarget(event.target);
  return [
    event.kind,
    event.level,
    event.app ?? "",
    event.domain ?? host ?? "",
    event.route ?? "",
    reasonKey(event.reason) ?? event.reason ?? "",
    event.kind === "other" ? event.detail ?? event.raw : "",
  ].join("|");
}

export function groupLog(events: LogEvent[]): LogRow[] {
  const rows = new Map<string, LogRow>();
  for (const event of events) {
    const key = rowKey(event);
    const { port } = splitTarget(event.target);
    const existing = rows.get(key);
    if (existing) {
      rows.delete(key);
      existing.count += 1;
      existing.event = event;
      existing.lastTime = event.time;
      if (port && !existing.ports.includes(port)) existing.ports.push(port);
      rows.set(key, existing);
    } else {
      rows.set(key, { key, event, count: 1, ports: port ? [port] : [], lastTime: event.time });
    }
  }
  return [...rows.values()];
}

export function stampLocalTime(event: LogEvent): LogEvent {
  return event.time ? event : { ...event, time: new Date().toLocaleTimeString() };
}
