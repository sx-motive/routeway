import {
  createContext,
  Fragment,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { en, type Dict, type Key, type Plural } from "./en";
import { ru } from "./ru";
import { zh } from "./zh";

export type Lang = "en" | "ru" | "zh";

export const LANGS: { id: Lang; label: string }[] = [
  { id: "en", label: "English" },
  { id: "ru", label: "Русский" },
  { id: "zh", label: "中文" },
];

export const DICTS: Record<Lang, Dict> = { en, ru, zh };

type Vars = Record<string, string | number>;

export function toLang(value: string | null | undefined): Lang {
  return value === "ru" || value === "zh" ? value : "en";
}

function fill(template: string, vars: Vars = {}): string {
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in vars ? String(vars[name]) : match,
  );
}

const pluralRules = new Map<Lang, Intl.PluralRules>();

function pickPlural(lang: Lang, forms: Plural, count: number): string {
  let rules = pluralRules.get(lang);
  if (!rules) {
    rules = new Intl.PluralRules(lang);
    pluralRules.set(lang, rules);
  }
  const category = rules.select(count) as keyof Plural;
  return forms[category] ?? forms.other;
}

export function translate(lang: Lang, key: Key, vars: Vars = {}): string {
  const entry = DICTS[lang][key] as string | Plural;
  if (typeof entry === "string") return fill(entry, vars);
  return fill(pickPlural(lang, entry, Number(vars.n ?? 0)), vars);
}

export function translateRich(lang: Lang, key: Key, nodes: Record<string, ReactNode>): ReactNode {
  const parts = translate(lang, key).split(/\{(\w+)\}/);
  return parts.map((part, index) =>
    index % 2 === 1 ? <Fragment key={index}>{nodes[part] ?? `{${part}}`}</Fragment> : part,
  );
}

interface I18nValue {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: (key: Key, vars?: Vars) => string;
  rich: (key: Key, nodes: Record<string, ReactNode>) => ReactNode;
}

const STORAGE_KEY = "routeway.lang";

function storedLang(): Lang {
  try {
    return toLang(localStorage.getItem(STORAGE_KEY));
  } catch {
    return "en";
  }
}

const I18nContext = createContext<I18nValue>({
  lang: "en",
  setLang: () => {},
  t: (key, vars) => translate("en", key, vars),
  rich: (key, nodes) => translateRich("en", key, nodes),
});

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(storedLang);

  useEffect(() => {
    document.documentElement.lang = lang === "zh" ? "zh-CN" : lang;
  }, [lang]);

  const setLang = useCallback((next: Lang) => {
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // storage can be unavailable; the language still applies for this session
    }
    setLangState(next);
  }, []);

  const value = useMemo<I18nValue>(
    () => ({
      lang,
      setLang,
      t: (key, vars) => translate(lang, key, vars),
      rich: (key, nodes) => translateRich(lang, key, nodes),
    }),
    [lang, setLang],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export const useI18n = () => useContext(I18nContext);
