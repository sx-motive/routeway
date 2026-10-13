import { describe, expect, it } from "vitest";
import { plural } from "./plural";

const DOMAIN: [string, string, string] = ["домен", "домена", "доменов"];

describe("plural", () => {
  it("picks the singular form for 1, 21, 101", () => {
    for (const n of [1, 21, 101, 131]) expect(plural(n, DOMAIN)).toBe("домен");
  });

  it("picks the few form for 2-4, 22-24", () => {
    for (const n of [2, 3, 4, 22, 33, 44]) expect(plural(n, DOMAIN)).toBe("домена");
  });

  it("picks the many form for 0, 5-20 and the teens", () => {
    for (const n of [0, 5, 9, 10, 11, 12, 14, 19, 20, 25, 111]) {
      expect(plural(n, DOMAIN)).toBe("доменов");
    }
  });
});
