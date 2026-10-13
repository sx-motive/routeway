import { describe, expect, it } from "vitest";
import { DICTS, translate } from "./index";
import type { Key, Plural } from "./en";

function placeholders(entry: string | Plural): string[] {
  const text = typeof entry === "string" ? entry : entry.other;
  return [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
}

describe("dictionaries", () => {
  const keys = Object.keys(DICTS.en) as Key[];

  it("use the same placeholders in every language", () => {
    for (const key of keys) {
      const expected = placeholders(DICTS.en[key]);
      expect(placeholders(DICTS.ru[key]), `ru ${key}`).toEqual(expected);
      expect(placeholders(DICTS.zh[key]), `zh ${key}`).toEqual(expected);
    }
  });

  it("have no empty strings", () => {
    for (const lang of ["en", "ru", "zh"] as const) {
      for (const key of keys) {
        const entry = DICTS[lang][key];
        const text = typeof entry === "string" ? entry : entry.other;
        expect(text.trim(), `${lang} ${key}`).not.toBe("");
      }
    }
  });
});

describe("plurals", () => {
  it("follow Russian 1 / 2-4 / 5-20 forms", () => {
    expect(translate("ru", "count.domains", { n: 1 })).toBe("1 домен");
    expect(translate("ru", "count.domains", { n: 21 })).toBe("21 домен");
    expect(translate("ru", "count.domains", { n: 3 })).toBe("3 домена");
    expect(translate("ru", "count.domains", { n: 5 })).toBe("5 доменов");
    expect(translate("ru", "count.domains", { n: 12 })).toBe("12 доменов");
  });

  it("follow English one / other", () => {
    expect(translate("en", "count.apps", { n: 1 })).toBe("1 app");
    expect(translate("en", "count.apps", { n: 2 })).toBe("2 apps");
  });

  it("use a single form in Chinese", () => {
    expect(translate("zh", "count.apps", { n: 3 })).toBe("3 个应用");
  });
});
