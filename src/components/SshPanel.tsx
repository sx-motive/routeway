import { useEffect, useRef, useState } from "react";
import { ArrowRight, Loader2, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useI18n } from "../i18n";
import { controlSsh, getSsh, saveSsh, type SshProfile, type SshSnapshot } from "../lib/ssh";
import { Button } from "./ui/button";
import { Card, Row } from "./ui/card";
import { Field, Input } from "./ui/input";
import { Switch } from "./ui/switch";

const emptyForward = () => ({ label: "", local_port: 15432, remote_host: "localhost", remote_port: 5432 });

export default function SshPanel() {
  const { t } = useI18n();
  const [snapshot, setSnapshot] = useState<SshSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<SshProfile | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const generation = useRef(0);
  const editor = useRef<HTMLFormElement>(null);

  useEffect(() => { if (editing) editor.current?.scrollIntoView?.({ block: "start" }); }, [editing?.id]);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      const version = generation.current;
      try {
        const next = await getSsh();
        if (!cancelled && version === generation.current) setSnapshot(next);
      } catch (err) {
        if (!cancelled && version === generation.current) setError(String(err));
      } finally {
        if (!cancelled) timer = setTimeout(() => void poll(), 1500);
      }
    }
    void poll();
    return () => { cancelled = true; clearTimeout(timer); };
  }, []);

  async function run(action: () => Promise<SshSnapshot>, closeEditor = false) {
    ++generation.current;
    setBusy(true);
    setError(null);
    try {
      const next = await action();
      ++generation.current;
      setSnapshot(next);
      if (closeEditor) setEditing(null);
    } catch (err) { setError(String(err)); }
    finally { ++generation.current; setBusy(false); }
  }

  const profiles = snapshot?.config.profiles ?? [];

  return <>
    <div className="flex items-start justify-between gap-3 pt-3">
      <div>
        <h1 className="text-sm font-medium text-fg">{t("ssh.title")}</h1>
        <p className="mt-1 text-xs leading-relaxed text-fg-muted">{t("ssh.description")}</p>
      </div>
      <Button size="sm" variant="outline" disabled={busy || !snapshot || !!snapshot.error || !!editing} onClick={() => setEditing({ id: crypto.randomUUID(), name: "", host: "", config_file: "", auto_connect: false, forwards: [emptyForward()] })}>
        <Plus className="h-3.5 w-3.5" />{t("ssh.add")}
      </Button>
    </div>
    {error || snapshot?.error ? <p role="alert" className="border border-danger/40 bg-danger/5 px-4 py-3 text-xs text-danger">{error ?? snapshot?.error}</p> : null}
    {!snapshot ? <p className="text-xs text-fg-muted">{t("app.loading")}</p> : null}
    {editing ? <Card title={t("ssh.profile")}>
      <form ref={editor} className="space-y-4 p-4" onSubmit={(event) => {
        event.preventDefault();
        const next = profiles.some((p) => p.id === editing.id) ? profiles.map((p) => p.id === editing.id ? editing : p) : [...profiles, editing];
        void run(() => saveSsh(next), true);
      }}>
        <Field label={t("ssh.name")}><Input required maxLength={120} value={editing.name} onChange={(e) => setEditing({ ...editing, name: e.target.value })} /></Field>
        <Field label={t("ssh.host")} hint={t("ssh.hostHint")}><Input required placeholder="user@server" value={editing.host} onChange={(e) => setEditing({ ...editing, host: e.target.value.trim() })} /></Field>
        <Field label={t("ssh.config")} hint={t("ssh.configHint")}><Input placeholder="~/.ssh/config" value={editing.config_file} onChange={(e) => setEditing({ ...editing, config_file: e.target.value })} /></Field>
        <div className="space-y-3">
          {editing.forwards.map((forward, index) => {
            function update(patch: Partial<typeof forward>) {
              setEditing({ ...editing!, forwards: editing!.forwards.map((f, i) => i === index ? { ...f, ...patch } : f) });
            }
            return <fieldset key={index} className="space-y-3 border border-border p-3">
              <div className="flex items-center justify-between gap-2">
                <span className="text-xs text-fg-muted">{t("ssh.forward")} {index + 1}</span>
                <Button type="button" variant="ghost" size="icon" aria-label={t("ssh.removeForward")} disabled={editing.forwards.length === 1} onClick={() => setEditing({ ...editing, forwards: editing.forwards.filter((_, i) => i !== index) })}><Trash2 className="h-3.5 w-3.5" /></Button>
              </div>
              <Field label={t("ssh.portLabel")}><Input maxLength={120} value={forward.label} onChange={(e) => update({ label: e.target.value })} /></Field>
              <div className="grid grid-cols-2 gap-3">
                <Field label={t("ssh.localPort")}><Input type="number" required min={1} max={65535} value={forward.local_port || ""} onChange={(e) => update({ local_port: Number(e.target.value) })} /></Field>
                <Field label={t("ssh.remotePort")}><Input type="number" required min={1} max={65535} value={forward.remote_port || ""} onChange={(e) => update({ remote_port: Number(e.target.value) })} /></Field>
              </div>
              <Field label={t("ssh.destination")}><Input required value={forward.remote_host} onChange={(e) => update({ remote_host: e.target.value.trim() })} /></Field>
            </fieldset>;
          })}
          <Button type="button" size="sm" variant="outline" disabled={editing.forwards.length >= 50} onClick={() => setEditing({ ...editing, forwards: [...editing.forwards, emptyForward()] })}><Plus className="h-3.5 w-3.5" />{t("ssh.addForward")}</Button>
        </div>
        <label className="flex items-center justify-between gap-3 text-xs"><span>{t("ssh.autoConnect")}</span><Switch checked={editing.auto_connect} onCheckedChange={(on) => setEditing({ ...editing, auto_connect: on })} /></label>
        <p className="text-xs leading-relaxed text-fg-muted">{t("ssh.saveHint")}</p>
        <div className="flex justify-end gap-2">
          <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => { setEditing(null); setDeleting(null); }}>{t("ssh.cancel")}</Button>
          <Button type="submit" size="sm" disabled={busy}>{busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}{t("ssh.save")}</Button>
        </div>
      </form>
    </Card> : null}
    {snapshot && profiles.length === 0 && !editing ? <Card><p className="px-4 py-6 text-xs leading-relaxed text-fg-muted">{t("ssh.empty")}</p></Card> : null}
    {profiles.map((profile) => {
      const live = snapshot?.states[profile.id];
      const phase = live?.phase ?? "stopped";
      const connected = phase === "connected";
      const requested = !!live?.requested;
      return <Card key={profile.id}>
        <div className="flex items-center gap-3 px-4 py-3">
          <span className={`h-2 w-2 shrink-0 ${connected ? "bg-ok" : requested ? "bg-warn pulse-dot" : "bg-fg-muted/40"}`} />
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-sm font-medium" title={profile.name}>{profile.name}</h2>
            <p role="status" className="mt-0.5 text-xs text-fg-muted">{t(`ssh.${phase}`)} <span className="text-fg-muted/60">· {profile.host}</span></p>
          </div>
          <Switch checked={requested} disabled={busy || !!editing} aria-label={t("ssh.toggle", { name: profile.name })} onCheckedChange={(on) => void run(() => controlSsh(profile.id, on ? "start" : "stop"))} />
        </div>
        <div className="divide-y divide-border border-t border-border">
          {profile.forwards.map((f) => <div key={f.local_port} className="px-4 py-2.5 text-xs">
            {f.label ? <p className="mb-1 text-fg">{f.label}</p> : null}
            <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-fg-muted">
              <span>127.0.0.1:<span className="text-fg">{f.local_port}</span></span><ArrowRight className="h-3 w-3" /><span className="break-all">{f.remote_host}:{f.remote_port}</span>
            </div>
          </div>)}
        </div>
        {live?.last_error ? <p className="break-words border-t border-border px-4 py-2.5 text-xs text-warn">{live.last_error}</p> : null}
        <Row label={t("ssh.autoConnect")} control={<Switch checked={profile.auto_connect} disabled={busy || !!editing} aria-label={t("ssh.autoConnectFor", { name: profile.name })} onCheckedChange={(on) => void run(() => saveSsh(profiles.map((p) => p.id === profile.id ? { ...p, auto_connect: on } : p)))} />} />
        <div className="flex flex-wrap items-center gap-1 border-t border-border px-3 py-2">
          <Button size="sm" variant="ghost" disabled={busy || !!editing || !requested} onClick={() => void run(() => controlSsh(profile.id, "restart"))}><RefreshCw className="h-3 w-3" />{t("ssh.restart")}</Button>
          <Button size="sm" variant="ghost" disabled={busy || !!editing} onClick={() => { setEditing(structuredClone(profile)); setDeleting(null); }}><Pencil className="h-3 w-3" />{t("ssh.edit")}</Button>
        </div>
        <details className="border-t border-border px-4 py-2 text-xs text-fg-muted">
          <summary className="cursor-pointer py-1">{t("ssh.log")}</summary>
          <pre className="thin-scroll max-h-48 overflow-auto whitespace-pre-wrap break-all py-2 text-[11px]">{live?.log.join("\n") || t("ssh.noLog")}</pre>
        </details>
        {editing?.id === profile.id ? <div className="space-y-2 border-t border-border px-4 py-2">
          {deleting === profile.id ? <>
            <p className="text-xs text-warn">{t("ssh.deleteConfirm", { name: profile.name })}</p>
            <div className="flex gap-2">
              <Button size="sm" variant="danger" disabled={busy} onClick={() => void run(() => saveSsh(profiles.filter((p) => p.id !== profile.id)), true)}>{t("ssh.delete")}</Button>
              <Button size="sm" variant="ghost" disabled={busy} onClick={() => setDeleting(null)}>{t("ssh.cancel")}</Button>
            </div>
          </> : <Button size="sm" variant="danger" disabled={busy} onClick={() => setDeleting(profile.id)}><Trash2 className="h-3 w-3" />{t("ssh.delete")}</Button>}
        </div> : null}
      </Card>;
    })}
    <p className="pb-2 text-xs leading-relaxed text-fg-muted">{t("ssh.footer")}</p>
  </>;
}
