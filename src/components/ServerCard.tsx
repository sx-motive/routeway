import { useState } from "react";
import { ClipboardPaste, Check, X } from "lucide-react";
import { Button } from "./ui/button";
import { Field, Input } from "./ui/input";
import { emptyServer, parseVlessLink, type Server } from "../lib/vless";

interface ServerCardProps {
  server: Server | null;
  onChange: (server: Server) => void;
}

export function ServerCard({ server, onChange }: ServerCardProps) {
  const [editing, setEditing] = useState(server === null);
  const [draft, setDraft] = useState<Server>(server ?? emptyServer());
  const [link, setLink] = useState("");
  const [error, setError] = useState<string | null>(null);

  function applyLink() {
    try {
      const parsed = parseVlessLink(link);
      setDraft(parsed);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  function save() {
    if (!draft.address.trim()) return setError("Укажите адрес сервера");
    if (!draft.uuid.trim()) return setError("Укажите UUID");
    setError(null);
    onChange({ ...draft, label: draft.label.trim() || draft.address.trim() });
    setEditing(false);
  }

  if (!editing && server) {
    return (
      <section className="border border-border">
        <header className="flex items-center justify-between border-b border-border px-3 py-2">
          <h2 className="text-xs font-medium tracking-wide text-fg-muted uppercase">Сервер</h2>
          <Button variant="ghost" size="sm" onClick={() => setEditing(true)}>
            Изменить
          </Button>
        </header>
        <div className="flex items-baseline gap-3 px-3 py-3">
          <span className="text-sm text-fg">{server.label}</span>
          <span className="font-mono text-xs text-fg-muted">
            {server.address}:{server.port}
          </span>
          {server.public_key ? (
            <span className="border border-border px-1.5 py-0.5 text-[10px] tracking-wide text-fg-muted uppercase">
              reality
            </span>
          ) : null}
        </div>
      </section>
    );
  }

  return (
    <section className="border border-border">
      <header className="flex items-center justify-between border-b border-border px-3 py-2">
        <h2 className="text-xs font-medium tracking-wide text-fg-muted uppercase">Сервер</h2>
        {server ? (
          <Button
            variant="ghost"
            size="icon"
            aria-label="Отменить"
            onClick={() => {
              setDraft(server);
              setEditing(false);
              setError(null);
            }}
          >
            <X className="h-3.5 w-3.5" />
          </Button>
        ) : null}
      </header>

      <div className="flex flex-col gap-3 px-3 py-3">
        <Field label="Вставить ссылку" hint="vless://…">
          <div className="flex gap-2">
            <Input
              value={link}
              onChange={(e) => setLink(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && applyLink()}
              placeholder="vless://uuid@host:port?security=reality&pbk=…"
              spellCheck={false}
              className="font-mono text-xs"
            />
            <Button variant="outline" size="sm" className="shrink-0" onClick={applyLink}>
              <ClipboardPaste className="h-3.5 w-3.5" />
              Разобрать
            </Button>
          </div>
        </Field>

        <div className="grid grid-cols-2 gap-3">
          <Field label="Название">
            <Input
              value={draft.label}
              onChange={(e) => setDraft({ ...draft, label: e.target.value })}
              placeholder="Эстония"
            />
          </Field>
          <Field label="Адрес">
            <Input
              value={draft.address}
              onChange={(e) => setDraft({ ...draft, address: e.target.value })}
              placeholder="198.51.100.7"
              spellCheck={false}
              className="font-mono text-xs"
            />
          </Field>
          <Field label="Порт">
            <Input
              type="number"
              value={draft.port}
              onChange={(e) => setDraft({ ...draft, port: Number(e.target.value) })}
              className="font-mono text-xs"
            />
          </Field>
          <Field label="UUID">
            <Input
              value={draft.uuid}
              onChange={(e) => setDraft({ ...draft, uuid: e.target.value })}
              spellCheck={false}
              className="font-mono text-xs"
            />
          </Field>
          <Field label="SNI">
            <Input
              value={draft.sni}
              onChange={(e) => setDraft({ ...draft, sni: e.target.value })}
              placeholder="www.cloudflare.com"
              spellCheck={false}
              className="font-mono text-xs"
            />
          </Field>
          <Field label="Flow">
            <Input
              value={draft.flow}
              onChange={(e) => setDraft({ ...draft, flow: e.target.value })}
              placeholder="xtls-rprx-vision"
              spellCheck={false}
              className="font-mono text-xs"
            />
          </Field>
          <Field label="Public key" hint="reality">
            <Input
              value={draft.public_key}
              onChange={(e) => setDraft({ ...draft, public_key: e.target.value })}
              spellCheck={false}
              className="font-mono text-xs"
            />
          </Field>
          <Field label="Short id" hint="reality">
            <Input
              value={draft.short_id}
              onChange={(e) => setDraft({ ...draft, short_id: e.target.value })}
              spellCheck={false}
              className="font-mono text-xs"
            />
          </Field>
        </div>

        {error ? <p className="text-xs text-danger">{error}</p> : null}

        <div className="flex justify-end">
          <Button size="sm" onClick={save}>
            <Check className="h-3.5 w-3.5" />
            Сохранить сервер
          </Button>
        </div>
      </div>
    </section>
  );
}
