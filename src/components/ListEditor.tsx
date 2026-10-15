import { useMemo, useState, type ReactNode } from "react";
import { Braces, Plus, X } from "lucide-react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Card } from "./ui/card";
import { useI18n } from "../i18n";

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
  onEditFile?: () => void;
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
  onEditFile,
}: ListEditorProps) {
  const { t } = useI18n();
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
    const seen = new Set(items);
    for (const candidate of candidates) {
      const value = normalize(candidate);
      if (!value) rejected.push(candidate);
      else if (!seen.has(value)) {
        seen.add(value);
        accepted.push(value);
      }
    }

    if (accepted.length > 0) onChange([...items, ...accepted]);
    setDraft(rejected.join(" "));
    setError(rejected.length > 0 ? `${rejectedLabel}: ${rejected.join(", ")}` : null);
  }

  return (
    <Card
      title={title}
      aside={
        <span className="-mr-2 flex items-center gap-1">
          <span className="text-xs text-fg-muted">
            {items.length === 0 ? t("list.empty") : items.length}
          </span>
          {onEditFile ? (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7"
              aria-label={t("rules.edit")}
              title={t("rules.edit")}
              onClick={onEditFile}
            >
              <Braces className="h-3.5 w-3.5" />
            </Button>
          ) : null}
        </span>
      }
    >
      <div className="flex gap-2 border-b border-border px-4 py-3">
        <Input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder={placeholder}
          spellCheck={false}
          className="font-mono text-xs"
        />
        <Button variant="outline" size="sm" className="h-9 shrink-0" onClick={add}>
          <Plus className="h-3.5 w-3.5" />
          {t("list.add")}
        </Button>
      </div>

      {error ? <p className="border-b border-border px-4 py-2 text-xs text-danger">{error}</p> : null}

      <div className="thin-scroll max-h-64 overflow-y-auto [scrollbar-gutter:auto]">
        {shown.length === 0 ? (
          <p className="px-4 py-6 text-center text-xs leading-relaxed text-fg-muted">{emptyHint}</p>
        ) : (
          <ul>
            {shown.map((item) => (
              <li
                key={item}
                className="group flex h-9 items-center justify-between border-b border-border/60 px-4 last:border-b-0 hover:bg-bg-elevated/50"
              >
                <span className="truncate font-mono text-xs text-fg">{item}</span>
                <button
                  type="button"
                  aria-label={t("list.remove", { item })}
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

      <footer className="border-t border-border px-4 py-2.5 text-xs leading-relaxed text-fg-muted">
        {footer}
      </footer>
    </Card>
  );
}
