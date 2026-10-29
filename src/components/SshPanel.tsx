import { useEffect, useRef, useState } from "react";
import { ArrowLeft, ArrowRight, FolderOpen, Loader2, Pencil, Plus, Trash2 } from "lucide-react";
import { useI18n } from "../i18n";
import { controlSsh, getSsh, revealSshConfig, saveSsh, type SshProfile, type SshSnapshot } from "../lib/ssh";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
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
  const header = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Scroll only the content pane; scrollIntoView also moves the window frame.
    if (header.current?.parentElement) header.current.parentElement.scrollTop = 0;
  }, [editing?.id]);

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
    <div ref={header} className="sticky top-0 z-10 space-y-2 border-b border-border bg-bg py-3">
      <div className="flex min-h-8 items-center justify-between gap-3">
        <h1 className="min-w-0 text-sm font-medium text-fg">{editing ? t("ssh.profile") : t("ssh.title")}</h1>
        {editing ? <div className="flex shrink-0 items-center gap-2">
          <Button size="sm" variant="outline" disabled={busy} onClick={() => { setEditing(null); setDeleting(null); }}><ArrowLeft className="h-3.5 w-3.5" />{t("ssh.back")}</Button>
          <Button type="submit" form="ssh-profile-editor" size="sm" disabled={busy}>{busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}{t("ssh.save")}</Button>
        </div> : <Button size="sm" variant="outline" className="bg-field text-fg hover:bg-bg-elevated" disabled={busy || !snapshot || !!snapshot.error} onClick={() => setEditing({ id: crypto.randomUUID(), name: "", host: "", config_file: "", auto_connect: false, forwards: [emptyForward()] })}>
          <Plus className="h-3.5 w-3.5" />{t("ssh.add")}
        </Button>}
      </div>

    </div>
    {error || snapshot?.error ? <p role="alert" className="border border-danger/40 bg-danger/5 px-4 py-3 text-xs text-danger">{error ?? snapshot?.error}</p> : null}
    {!snapshot ? <p className="text-xs text-fg-muted">{t("app.loading")}</p> : null}
    {editing ? <Card>
      <form id="ssh-profile-editor" className="space-y-3 p-3 [&_input]:h-8 [&_input]:px-2 [&_input]:text-xs [&_label]:gap-1" onSubmit={(event) => {
        event.preventDefault();
        const next = profiles.some((p) => p.id === editing.id) ? profiles.map((p) => p.id === editing.id ? editing : p) : [...profiles, editing];
        void run(() => saveSsh(next), true);
      }}>
        <div className="grid grid-cols-2 gap-3">
          <Field label={t("ssh.name")}><Input required maxLength={120} value={editing.name} onChange={(e) => setEditing({ ...editing, name: e.target.value })} /></Field>
          <Field label={t("ssh.host")}><Input required title={t("ssh.hostHint")} placeholder="user@server" value={editing.host} onChange={(e) => setEditing({ ...editing, host: e.target.value.trim() })} /></Field>
        </div>
        <Field label={t("ssh.config")}><span className="flex items-center gap-2"><Input title={t("ssh.configHint")} placeholder="~/.ssh/config" value={editing.config_file} onChange={(e) => setEditing({ ...editing, config_file: e.target.value })} /><Button type="button" size="icon" variant="outline" aria-label={t("rules.reveal")} title={t("rules.reveal")} onClick={() => { setError(null); void revealSshConfig(editing.config_file).catch((err) => setError(String(err))); }}><FolderOpen className="h-3.5 w-3.5" /></Button></span></Field>
        <div className="space-y-3">
          {editing.forwards.map((forward, index) => {
            function update(patch: Partial<typeof forward>) {
              setEditing({ ...editing!, forwards: editing!.forwards.map((f, i) => i === index ? { ...f, ...patch } : f) });
            }
            return <fieldset key={index} className="min-w-0 border border-border p-2">
              <div className="grid grid-cols-[minmax(0,1.1fr)_minmax(0,.75fr)_1rem_minmax(0,1.2fr)_minmax(0,.75fr)_2rem] items-center gap-1.5 [&_input]:min-w-0 [&_input]:tabular-nums">
                <Input aria-label={t("ssh.portLabel")} title={t("ssh.portLabel")} placeholder={t("ssh.portPlaceholder", { n: index + 1 })} maxLength={120} value={forward.label} onChange={(e) => update({ label: e.target.value })} />
                <Input aria-label={t("ssh.localPort")} title={t("ssh.localPort")} type="text" inputMode="numeric" pattern="[0-9]+" maxLength={5} required value={forward.local_port || ""} onChange={(e) => update({ local_port: Number(e.target.value.replace(/[^0-9]/g, "")) })} />
                <ArrowRight aria-hidden="true" className="h-4 w-4 text-fg-muted" />
                <Input aria-label={t("ssh.destination")} title={t("ssh.destination")} required value={forward.remote_host} onChange={(e) => update({ remote_host: e.target.value.trim() })} />
                <Input aria-label={t("ssh.remotePort")} title={t("ssh.remotePort")} type="text" inputMode="numeric" pattern="[0-9]+" maxLength={5} required value={forward.remote_port || ""} onChange={(e) => update({ remote_port: Number(e.target.value.replace(/[^0-9]/g, "")) })} />
                <Button type="button" variant="ghost" size="icon" aria-label={t("ssh.removeForward")} disabled={editing.forwards.length === 1} onClick={() => setEditing({ ...editing, forwards: editing.forwards.filter((_, i) => i !== index) })}><Trash2 className="h-3.5 w-3.5" /></Button>
              </div>
            </fieldset>;
          })}
          <Button type="button" size="sm" variant="outline" disabled={editing.forwards.length >= 50} onClick={() => setEditing({ ...editing, forwards: [...editing.forwards, emptyForward()] })}><Plus className="h-3.5 w-3.5" />{t("ssh.addForward")}</Button>
        </div>
        <label className="flex items-center justify-between gap-3 text-xs"><span>{t("ssh.autoConnect")}</span><Switch checked={editing.auto_connect} onCheckedChange={(on) => setEditing({ ...editing, auto_connect: on })} /></label>
        <p className="text-xs leading-relaxed text-fg-muted">{t("ssh.saveHint")}</p>
        {snapshot?.states[editing.id]?.last_error ? <p className="break-words border-t border-border pt-2.5 text-xs text-warn">{snapshot?.states[editing.id]?.last_error}</p> : null}
        <details className="border-t border-border pt-2 text-xs text-fg-muted">
          <summary className="cursor-pointer py-1">{t("ssh.log")}</summary>
          <pre className="thin-scroll max-h-48 overflow-auto whitespace-pre-wrap break-all py-2 text-[11px]">{snapshot?.states[editing.id]?.log.join("\n") || t("ssh.noLog")}</pre>
        </details>
        {profiles.some((p) => p.id === editing.id) ? <div className="space-y-2 border-t border-border pt-2">
          {deleting === editing.id ? <>
            <p className="text-xs text-warn">{t("ssh.deleteConfirm", { name: editing.name })}</p>
            <div className="flex gap-2">
              <Button type="button" size="sm" variant="danger" disabled={busy} onClick={() => void run(() => saveSsh(profiles.filter((p) => p.id !== editing.id)), true)}>{t("ssh.delete")}</Button>
              <Button type="button" size="sm" variant="ghost" disabled={busy} onClick={() => setDeleting(null)}>{t("ssh.cancel")}</Button>
            </div>
          </> : <Button type="button" size="sm" variant="danger" disabled={busy} onClick={() => setDeleting(editing.id)}><Trash2 className="h-3 w-3" />{t("ssh.delete")}</Button>}
        </div> : null}
      </form>
    </Card> : null}
    {snapshot && profiles.length === 0 && !editing ? <Card><p className="px-4 py-6 text-xs leading-relaxed text-fg-muted">{t("ssh.empty")}</p></Card> : null}
    {!editing && profiles.map((profile) => {
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
          <Button size="icon" variant="ghost" disabled={busy} aria-label={`${t("ssh.edit")}: ${profile.name}`} onClick={() => { setEditing(structuredClone(profile)); setDeleting(null); }}><Pencil className="h-3.5 w-3.5" /></Button>
          <Switch checked={requested} disabled={busy || !!editing} aria-label={t("ssh.toggle", { name: profile.name })} onCheckedChange={(on) => void run(() => controlSsh(profile.id, on ? "start" : "stop"))} />
        </div>
      </Card>;
    })}

  </>;
}
