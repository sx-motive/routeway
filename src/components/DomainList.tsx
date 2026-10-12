import { useMemo, useState } from "react";
import { Plus, X } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

interface DomainListProps {
  domains: string[];
  onChange: (domains: string[]) => void;
}

/** Mirrors the Rust-side normalizer so the UI shows what will actually be stored. */
export function normalizeDomain(raw: string): string | null {
  let value = raw.trim().toLowerCase();
  if (!value || value.startsWith("#")) return null;
  const schemeAt = value.indexOf("://");
  if (schemeAt >= 0) value = value.slice(schemeAt + 3);
  value = value.split("/")[0].split("?")[0];
  const at = value.lastIndexOf("@");
  if (at >= 0) value = value.slice(at + 1);
  const colon = value.indexOf(":");
  if (colon >= 0) value = value.slice(0, colon);
  value = value.replace(/^\.+|\.+$/g, "");
  if (!value || !value.includes(".")) return null;
  return value;
}

export function DomainList({ domains, onChange }: DomainListProps) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const sorted = useMemo(() => [...domains].sort((a, b) => a.localeCompare(b)), [domains]);

  function add() {
    // One paste can carry a whole list — split on commas, spaces and newlines.
    const candidates = draft.split(/[\s,;]+/).filter(Boolean);
    if (candidates.length === 0) return;

    const accepted: string[] = [];
    const rejected: string[] = [];
    for (const candidate of candidates) {
      const domain = normalizeDomain(candidate);
      if (!domain) rejected.push(candidate);
      else if (!domains.includes(domain) && !accepted.includes(domain)) accepted.push(domain);
    }

    if (accepted.length > 0) onChange([...domains, ...accepted]);
    setDraft(rejected.join(" "));
    setError(rejected.length > 0 ? `Не похоже на домен: ${rejected.join(", ")}` : null);
  }

  return (
    <section className="flex flex-col border border-border">
      <header className="flex items-center justify-between border-b border-border px-3 py-2">
        <h2 className="text-xs font-medium tracking-wide text-fg-muted uppercase">
          Домены через сервер
        </h2>
        <span className="text-xs text-fg-muted/60">
          {domains.length === 0 ? "список пуст" : `${domains.length}`}
        </span>
      </header>

      <div className="flex gap-2 border-b border-border px-3 py-3">
        <Input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder="youtube.com"
          spellCheck={false}
          className="font-mono text-xs"
        />
        <Button variant="outline" size="sm" className="shrink-0" onClick={add}>
          <Plus className="h-3.5 w-3.5" />
          Добавить
        </Button>
      </div>

      {error ? <p className="px-3 pt-2 text-xs text-danger">{error}</p> : null}

      <div className="thin-scroll max-h-64 overflow-y-auto">
        {sorted.length === 0 ? (
          <p className="px-3 py-6 text-center text-xs text-fg-muted/60">
            Пока весь трафик идёт напрямую.
            <br />
            Добавь домен — и только он пойдёт через сервер.
          </p>
        ) : (
          <ul>
            {sorted.map((domain) => (
              <li
                key={domain}
                className="group flex items-center justify-between border-b border-border/50 px-3 py-1.5 last:border-b-0"
              >
                <span className="font-mono text-xs text-fg">{domain}</span>
                <button
                  type="button"
                  aria-label={`Убрать ${domain}`}
                  className="text-fg-muted/40 opacity-0 transition-opacity group-hover:opacity-100 hover:text-danger"
                  onClick={() => onChange(domains.filter((d) => d !== domain))}
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <footer className="border-t border-border px-3 py-2 text-[11px] text-fg-muted/60">
        Домен включает все поддомены: <span className="font-mono">youtube.com</span> покрывает и{" "}
        <span className="font-mono">www.youtube.com</span>.
      </footer>
    </section>
  );
}
