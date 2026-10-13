import { useMemo, useState, type ReactNode } from "react";
import { Plus, X } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

interface ListEditorProps {
  title: string;
  placeholder: string;
  items: string[];
  onChange: (items: string[]) => void;
  /** Returns the canonical form of an entry, or null if it is not valid. */
  normalize: (raw: string) => string | null;
  rejectedLabel: string;
  emptyHint: ReactNode;
  footer: ReactNode;
  /** Entries are shown sorted unless insertion order carries meaning. */
  sorted?: boolean;
}

export function ListEditor({
  title,
  placeholder,
  items,
  onChange,
  normalize,
  rejectedLabel,
  emptyHint,
  footer,
  sorted = true,
}: ListEditorProps) {
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const shown = useMemo(
    () => (sorted ? [...items].sort((a, b) => a.localeCompare(b)) : items),
    [items, sorted],
  );

  function add() {
    // One paste can carry a whole list — split on commas, spaces and newlines.
    const candidates = draft.split(/[\s,;]+/).filter(Boolean);
    if (candidates.length === 0) return;

    const accepted: string[] = [];
    const rejected: string[] = [];
    for (const candidate of candidates) {
      const value = normalize(candidate);
      if (!value) rejected.push(candidate);
      else if (!items.includes(value) && !accepted.includes(value)) accepted.push(value);
    }

    if (accepted.length > 0) onChange([...items, ...accepted]);
    setDraft(rejected.join(" "));
    setError(rejected.length > 0 ? `${rejectedLabel}: ${rejected.join(", ")}` : null);
  }

  return (
    <section className="flex flex-col border border-border">
      <header className="flex items-center justify-between border-b border-border px-3 py-2">
        <h2 className="text-xs font-medium tracking-wide text-fg-muted uppercase">{title}</h2>
        <span className="text-xs text-fg-muted/60">
          {items.length === 0 ? "список пуст" : items.length}
        </span>
      </header>

      <div className="flex gap-2 border-b border-border px-3 py-3">
        <Input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder={placeholder}
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
        {shown.length === 0 ? (
          <p className="px-3 py-6 text-center text-xs text-fg-muted/60">{emptyHint}</p>
        ) : (
          <ul>
            {shown.map((item) => (
              <li
                key={item}
                className="group flex items-center justify-between border-b border-border/50 px-3 py-1.5 last:border-b-0"
              >
                <span className="truncate font-mono text-xs text-fg">{item}</span>
                <button
                  type="button"
                  aria-label={`Убрать ${item}`}
                  className="shrink-0 pl-2 text-fg-muted/40 opacity-0 transition-opacity group-hover:opacity-100 hover:text-danger"
                  onClick={() => onChange(items.filter((i) => i !== item))}
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <footer className="border-t border-border px-3 py-2 text-[11px] text-fg-muted/60">
        {footer}
      </footer>
    </section>
  );
}
