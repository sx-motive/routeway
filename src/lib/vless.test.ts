import { describe, expect, it } from "vitest";
import { parseVlessLink, toVlessLink } from "./vless";

const REALITY_LINK =
  "vless://11111111-2222-3333-4444-555555555555@198.51.100.7:47443" +
  "?type=tcp&security=reality&sni=www.cloudflare.com&fp=chrome" +
  "&pbk=TESTPUBLICKEY&sid=a1b2&flow=xtls-rprx-vision&spx=%2F#Estonia";

describe("parseVlessLink", () => {
  it("reads every field of a reality link", () => {
    const server = parseVlessLink(REALITY_LINK);
    expect(server).toEqual({
      label: "Estonia",
      address: "198.51.100.7",
      port: 47443,
      uuid: "11111111-2222-3333-4444-555555555555",
      flow: "xtls-rprx-vision",
      sni: "www.cloudflare.com",
      public_key: "TESTPUBLICKEY",
      short_id: "a1b2",
      fingerprint: "chrome",
    });
  });

  it("defaults the label to the address and the fingerprint to chrome", () => {
    const server = parseVlessLink("vless://uuid-only@example.org:443?security=tls");
    expect(server.label).toBe("example.org");
    expect(server.fingerprint).toBe("chrome");
    expect(server.public_key).toBe("");
  });

  it("accepts peer= as an alias for sni", () => {
    const server = parseVlessLink("vless://u@example.org:443?peer=cdn.example.net");
    expect(server.sni).toBe("cdn.example.net");
  });

  it("rejects links that are not vless", () => {
    expect(() => parseVlessLink("ss://whatever")).toThrow("vless.scheme");
    expect(() => parseVlessLink("   ")).toThrow("vless.empty");
  });

  it("rejects a link without a uuid", () => {
    expect(() => parseVlessLink("vless://example.org:443")).toThrow("vless.noUuid");
  });

  it("rejects an out-of-range port", () => {
    expect(() => parseVlessLink("vless://u@example.org:70000")).toThrow();
  });
});

describe("toVlessLink", () => {
  it("round-trips a reality server", () => {
    const parsed = parseVlessLink(REALITY_LINK);
    expect(parseVlessLink(toVlessLink(parsed))).toEqual(parsed);
  });

  it("marks a keyless server as plain tls", () => {
    const parsed = parseVlessLink(REALITY_LINK);
    const link = toVlessLink({ ...parsed, public_key: "", short_id: "" });
    expect(link).toContain("security=tls");
    expect(link).not.toContain("pbk=");
  });
});
