import type { Key } from "../i18n/en";

export interface Server {
  label: string;
  address: string;
  port: number;
  uuid: string;
  flow: string;
  sni: string;
  public_key: string;
  short_id: string;
  fingerprint: string;
}

export function emptyServer(): Server {
  return {
    label: "",
    address: "",
    port: 443,
    uuid: "",
    flow: "",
    sni: "",
    public_key: "",
    short_id: "",
    fingerprint: "chrome",
  };
}

export class VlessError extends Error {
  constructor(readonly key: Key) {
    super(key);
  }
}

/**
 * Parses a `vless://` share link into the fields Routeway stores.
 *
 * Shape: vless://<uuid>@<host>:<port>?security=reality&sni=…&pbk=…&sid=…&flow=…#<label>
 * Unknown query parameters are ignored rather than rejected — share links in the
 * wild carry plenty of extras (type, encryption, headerType, spx…).
 */
export function parseVlessLink(input: string): Server {
  const raw = input.trim();
  if (!raw) throw new VlessError("vless.empty");
  if (!raw.toLowerCase().startsWith("vless://")) {
    throw new VlessError("vless.scheme");
  }

  let url: URL;
  try {
    // URL understands the vless: scheme well enough once it has the // authority.
    url = new URL(raw);
  } catch {
    throw new VlessError("vless.parse");
  }

  const uuid = decodeURIComponent(url.username || "");
  if (!uuid) throw new VlessError("vless.noUuid");

  const address = url.hostname.replace(/^\[|\]$/g, "");
  if (!address) throw new VlessError("vless.noAddress");

  const port = Number(url.port || "443");
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new VlessError("vless.badPort");
  }

  const q = url.searchParams;
  const label = decodeURIComponent((url.hash || "").replace(/^#/, "")).trim();

  return {
    label: label || address,
    address,
    port,
    uuid,
    flow: q.get("flow") ?? "",
    sni: q.get("sni") ?? q.get("peer") ?? "",
    public_key: q.get("pbk") ?? "",
    short_id: q.get("sid") ?? "",
    fingerprint: q.get("fp") || "chrome",
  };
}

/** Renders a server back into a share link, for copying between machines. */
export function toVlessLink(server: Server): string {
  const q = new URLSearchParams({ type: "tcp", encryption: "none" });
  if (server.public_key) {
    q.set("security", "reality");
    q.set("pbk", server.public_key);
    if (server.short_id) q.set("sid", server.short_id);
  } else {
    q.set("security", "tls");
  }
  if (server.sni) q.set("sni", server.sni);
  if (server.flow) q.set("flow", server.flow);
  if (server.fingerprint) q.set("fp", server.fingerprint);

  const host = server.address.includes(":") ? `[${server.address}]` : server.address;
  const label = server.label ? `#${encodeURIComponent(server.label)}` : "";
  return `vless://${server.uuid}@${host}:${server.port}?${q.toString()}${label}`;
}
