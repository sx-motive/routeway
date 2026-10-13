import { useState } from "react";
import { ClipboardPaste, Check, X } from "lucide-react";
import { Button } from "./ui/button";
import { Field, Input } from "./ui/input";
import { Card } from "./ui/card";
import { NumericInput } from "./ui/numeric-input";
import { emptyServer, parseVlessLink, VlessError, type Server } from "../lib/vless";
import { useI18n } from "../i18n";

interface ServerCardProps {
  server: Server | null;
  onChange: (server: Server) => void;
}

export function ServerCard({ server, onChange }: ServerCardProps) {
  const { t } = useI18n();
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
      setError(err instanceof VlessError ? t(err.key) : String(err));
    }
  }

  function save() {
    if (!draft.address.trim()) return setError(t("server.needAddress"));
    if (!draft.uuid.trim()) return setError(t("server.needUuid"));
    setError(null);
    onChange({ ...draft, label: draft.label.trim() || draft.address.trim() });
    setEditing(false);
  }

  if (!editing && server) {
    return (
      <Card
        title={t("server.title")}
        aside={
          <Button variant="ghost" size="sm" className="-mr-2 h-7" onClick={() => setEditing(true)}>
            {t("server.edit")}
          </Button>
        }
      >
        <div className="flex min-h-14 items-center gap-3 px-4 py-3">
          <span className="truncate text-sm text-fg">{server.label}</span>
          <span className="truncate font-mono text-xs text-fg-muted">
            {server.address}:{server.port}
          </span>
          {server.public_key ? (
            <span className="ml-auto shrink-0 border border-border px-1.5 py-0.5 text-[10px] tracking-wider text-fg-muted uppercase">
              reality
            </span>
          ) : null}
        </div>
      </Card>
    );
  }

  return (
    <Card
      title={t("server.title")}
      aside={
        server ? (
          <Button
            variant="ghost"
            size="icon"
            className="-mr-2 h-7 w-7"
            aria-label={t("server.cancel")}
            onClick={() => {
              setDraft(server);
              setEditing(false);
              setError(null);
            }}
          >
            <X className="h-3.5 w-3.5" />
          </Button>
        ) : null
      }
    >
      <div className="flex flex-col gap-4 px-4 py-4">
        <Field label={t("server.pasteLink")} hint="vless://…">
          <div className="flex gap-2">
            <Input
              value={link}
              onChange={(e) => setLink(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && applyLink()}
              placeholder="vless://uuid@host:port?security=reality&pbk=…"
              spellCheck={false}
              className="font-mono text-xs"
            />
            <Button variant="outline" size="sm" className="h-9 shrink-0" onClick={applyLink}>
              <ClipboardPaste className="h-3.5 w-3.5" />
              {t("server.parse")}
            </Button>
          </div>
        </Field>

        <div className="grid grid-cols-2 gap-x-3 gap-y-4">
          <Field label={t("server.name")}>
            <Input
              value={draft.label}
              onChange={(e) => setDraft({ ...draft, label: e.target.value })}
              placeholder={t("server.namePlaceholder")}
            />
          </Field>
          <Field label={t("server.address")}>
            <Input
              value={draft.address}
              onChange={(e) => setDraft({ ...draft, address: e.target.value })}
              placeholder="198.51.100.7"
              spellCheck={false}
              className="font-mono text-xs"
            />
          </Field>
          <Field label={t("server.port")}>
            <NumericInput
              min={1}
              max={65535}
              value={draft.port}
              onValueChange={(port) => setDraft({ ...draft, port })}
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
          <Button size="sm" className="h-9" onClick={save}>
            <Check className="h-3.5 w-3.5" />
            {t("server.save")}
          </Button>
        </div>
      </div>
    </Card>
  );
}
