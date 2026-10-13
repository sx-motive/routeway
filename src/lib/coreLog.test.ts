import { describe, expect, it } from "vitest";
import { groupLog, reasonKey, splitTarget, type LogEvent } from "./coreLog";

function failure(time: string, target: string, app: string | null = null): LogEvent {
  return {
    time,
    level: "error",
    kind: "connection_failed",
    app,
    target,
    domain: null,
    route: "direct",
    reason: "i/o timeout",
    detail: null,
    raw: `${time} raw`,
  };
}

describe("groupLog", () => {
  it("folds repeats of the same failure across ports into one row", () => {
    const rows = groupLog([
      failure("16:32:10", "149.154.167.35:80"),
      failure("16:32:10", "149.154.167.35:443"),
      failure("16:32:15", "149.154.167.35:80"),
    ]);
    expect(rows).toHaveLength(1);
    expect(rows[0].count).toBe(3);
    expect(rows[0].ports).toEqual(["80", "443"]);
    expect(rows[0].lastTime).toBe("16:32:15");
  });

  it("keeps different apps apart and moves the latest repeat to the end", () => {
    const rows = groupLog([
      failure("1", "1.1.1.1:443", "Telegram.exe"),
      failure("2", "1.1.1.1:443", "chrome.exe"),
      failure("3", "1.1.1.1:443", "Telegram.exe"),
    ]);
    expect(rows.map((r) => r.event.app)).toEqual(["chrome.exe", "Telegram.exe"]);
  });
});

describe("reasonKey", () => {
  it("maps Go network errors to plain categories", () => {
    expect(reasonKey("i/o timeout")).toBe("timeout");
    expect(reasonKey("connectex: No connection could be made because the target machine actively refused it.")).toBe("refused");
    expect(reasonKey("wsarecv: An existing connection was forcibly closed by the remote host.")).toBe("reset");
    expect(reasonKey("lookup x: no such host")).toBe("notFound");
    expect(reasonKey("something odd")).toBeNull();
  });
});

describe("splitTarget", () => {
  it("separates host and port, leaving bare IPv6 alone", () => {
    expect(splitTarget("example.org:443")).toEqual({ host: "example.org", port: "443" });
    expect(splitTarget("2001:db8::1")).toEqual({ host: "2001:db8::1", port: null });
  });
});
