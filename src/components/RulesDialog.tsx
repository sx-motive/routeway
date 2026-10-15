import { useEffect, useMemo, useRef, useState } from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { Check, FolderOpen, Loader2, X } from "lucide-react";
import { Button } from "./ui/button";
import { getRules, revealRules, saveRules, type Status } from "../lib/ipc";
import { useI18n } from "../i18n";

const INDENT = "  ";

function jsonProblem(text: string, notObject: string): string | null {
  try {
    const value: unknown = JSON.parse(text);
    if (typeof value !== "object" || value === null || Array.isArray(value)) return notObject;
    return null;
  } catch (err) {
    return err instanceof Error ? err.message : String(err);
  }
}

export function RulesDialog({
  open,
  onOpenChange,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSaved: (status: Status) => void;
}) {
  const { t } = useI18n();
  const [text, setText] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  const areaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoaded(false);
    setServerError(null);
    void getRules().then((raw) => {
      if (cancelled) return;
      setText(raw);
      setLoaded(true);
    }).catch((err) => {
      if (!cancelled) setServerError(String(err));
    });
    return () => { cancelled = true; };
  }, [open]);

  const problem = useMemo(() => (loaded ? jsonProblem(text, t("rules.notObject")) : null), [text, loaded, t]);
  const error = problem ? t("rules.invalid", { error: problem }) : serverError;

  async function save() {
    if (!loaded || problem || saving) return;
    setSaving(true);
    try {
      onSaved(await saveRules(text));
      onOpenChange(false);
    } catch (err) {
      setServerError(String(err));
    } finally {
      setSaving(false);
    }
  }

  function indentOnTab(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void save();
      return;
    }
    if (e.key !== "Tab") return;
    e.preventDefault();
    const area = e.currentTarget;
    const { selectionStart: start, selectionEnd: end } = area;
    const next = text.slice(0, start) + INDENT + text.slice(end);
    setText(next);
    setServerError(null);
    requestAnimationFrame(() => {
      area.selectionStart = area.selectionEnd = start + INDENT.length;
    });
  }

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content
          onOpenAutoFocus={(e) => {
            e.preventDefault();
            areaRef.current?.focus();
          }}
          className="fixed inset-4 z-50 m-auto flex max-h-[640px] max-w-[720px] flex-col border border-border bg-surface focus:outline-none"
        >
          <header className="flex h-10 shrink-0 items-center justify-between gap-3 border-b border-border px-4">
            <Dialog.Title className="text-[11px] font-medium tracking-wider text-fg-muted uppercase">
              {t("rules.title")}
            </Dialog.Title>
            <Dialog.Close asChild>
              <Button variant="ghost" size="icon" className="-mr-2 h-7 w-7" aria-label={t("server.cancel")}>
                <X className="h-3.5 w-3.5" />
              </Button>
            </Dialog.Close>
          </header>

          <Dialog.Description className="border-b border-border px-4 py-2.5 text-xs leading-relaxed text-fg-muted">
            {t("rules.hint")}
          </Dialog.Description>

          <textarea
            ref={areaRef}
            value={text}
            disabled={!loaded}
            onChange={(e) => {
              setText(e.target.value);
              setServerError(null);
            }}
            onKeyDown={indentOnTab}
            spellCheck={false}
            autoCapitalize="off"
            autoComplete="off"
            className="thin-scroll min-h-0 flex-1 resize-none bg-field px-4 py-3 font-mono text-xs leading-relaxed text-fg focus:outline-none"
          />

          <footer className="flex shrink-0 items-center gap-3 border-t border-border px-4 py-3">
            <Button
              variant="ghost"
              size="sm"
              className="-ml-2 h-9 shrink-0"
              title="rules.json"
              onClick={() => void revealRules().catch((err) => setServerError(String(err)))}
            >
              <FolderOpen className="h-3.5 w-3.5" />
              {t("rules.reveal")}
            </Button>
            <p className="min-w-0 flex-1 truncate text-xs text-danger" title={error ?? undefined}>
              {error}
            </p>
            <Dialog.Close asChild>
              <Button variant="outline" size="sm" className="h-9">
                {t("server.cancel")}
              </Button>
            </Dialog.Close>
            <Button size="sm" className="h-9" disabled={!loaded || !!problem || saving} onClick={save}>
              {saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
              {t("rules.save")}
            </Button>
          </footer>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
