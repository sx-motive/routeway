import { useEffect, useRef, useState } from "react";
import { FolderOpen, Loader2, Power, RefreshCw, RotateCcw, Square } from "lucide-react";
import { Card, Row, Rows } from "./ui/card";
import { Button } from "./ui/button";
import { useI18n } from "../i18n";
import { getDiagnostics, revealConfig, type Diagnostics, type MaintenanceAction } from "../lib/ipc";

export function DiagnosticsPanel({ working, canRestart, maintainCore }: {
  working: boolean;
  canRestart: boolean;
  maintainCore: (action: MaintenanceAction) => Promise<void>;
}) {
  const { t } = useI18n();
  const [data, setData] = useState<Diagnostics | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [revision, setRevision] = useState(0);
  const [pending, setPending] = useState<MaintenanceAction | null>(null);
  const acting = useRef(false);

  useEffect(() => {
    if (working) return;
    let cancelled = false;
    setLoading(true);
    void getDiagnostics().then((snapshot) => {
      if (!cancelled) { setData(snapshot); setError(null); }
    }).catch((err) => {
      if (!cancelled) setError(String(err));
    }).finally(() => {
      if (!cancelled) setLoading(false);
    });
    return () => { cancelled = true; };
  }, [working, revision]);

  async function run(action: MaintenanceAction) {
    if (acting.current || working) return;
    acting.current = true;
    setPending(action);
    try {
      await maintainCore(action);
    } catch (err) {
      setError(String(err));
    } finally {
      acting.current = false;
      setPending(null);
      setRevision((value) => value + 1);
    }
  }

  const disabled = working || pending !== null;
  return (
    <Card title={t("debug.title")} aside={
        <Button variant="ghost" size="sm" className="-mr-2 h-7" disabled={disabled || loading}
          onClick={() => setRevision((value) => value + 1)}>
          {loading ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
          {t("debug.refresh")}
        </Button>
    }>
      {error ? <p role="alert" className="break-words border-b border-border px-4 py-3 text-xs leading-relaxed text-danger">{error}</p> : null}
        {data ? <>
          <dl className="grid grid-cols-3 divide-x divide-border border-b border-border py-3 text-xs">
            <div className="px-4"><dt className="text-fg-muted">{t("debug.core")}</dt><dd className="mt-1 flex items-center gap-1.5 text-fg"><span className={`size-1.5 shrink-0 ${data.core_running ? "bg-ok" : "bg-fg-muted"}`} />{t(data.core_running ? "debug.running" : "debug.stopped")}</dd></div>
            <div className="px-4"><dt className="text-fg-muted">{t("debug.proxy")}</dt><dd className="mt-1 text-fg">{t(data.system_proxy_active ? "debug.on" : "debug.off")}</dd></div>
            <div className="px-4"><dt className="text-fg-muted">{t("debug.port")}</dt><dd className="mt-1 font-mono text-fg">{data.listen_port}</dd></div>
          </dl>
          <div className="px-4 py-3">
            <table className="w-full table-fixed text-left text-xs">
              <caption className="sr-only">{t("debug.hint")}</caption>
              <thead className="border-b border-border text-fg-muted"><tr>
                <th className="pb-2 font-normal">{t("debug.processes")}</th>
                <th className="w-16 pb-2 text-right font-normal">PID</th>
                <th className="w-16 pb-2 text-right font-normal" title={t("debug.parent")}>PPID</th>
              </tr></thead>
              <tbody className="divide-y divide-border text-fg">
                {data.processes.map((process) => <tr key={process.pid}>
                  <td className="py-2.5 pr-3">
                    <span className="block truncate font-mono text-xs" title={process.name}>{process.name}</span>
                    <span className="mt-0.5 block text-xs text-fg-muted">{t(`debug.role.${process.role}`)}</span>
                  </td>
                  <td className="py-2.5 text-right font-mono tabular-nums">{process.pid}</td>
                  <td className="py-2.5 text-right font-mono tabular-nums text-fg-muted">{process.parent_pid}</td>
                </tr>)}
              </tbody>
            </table>
          </div>
        </> : loading ? <p role="status" className="px-4 py-3 text-xs text-fg-muted">{t("app.loading")}</p> : null}
        <div className="border-t border-border">
          <Rows>
          {([
            { action: "restart", label: "debug.restartCore", hint: "debug.restartHint", Icon: RotateCcw },
            { action: "clean", label: "debug.clean", hint: "debug.cleanHint", Icon: Square },
            { action: "app", label: "debug.restartApp", hint: "debug.appHint", Icon: Power },
          ] as const).map(({ action, label, hint, Icon }) => (
            <Row key={action} interactive={false} label={t(label)} hint={t(hint)} control={
              <Button variant={action === "clean" ? "danger" : "outline"} size="icon"
                aria-label={t(label)} title={t(label)}
                disabled={disabled || (action === "restart" && !canRestart)} onClick={() => void run(action)}>
                {pending === action ? <Loader2 className="size-3.5 animate-spin" /> : <Icon className="size-3.5" />}
              </Button>
            } />
          ))}
          <Row interactive={false} label={t("debug.folder")} control={
            <Button variant="ghost" size="icon" aria-label={t("debug.folder")} title={t("debug.folder")}
              disabled={disabled} onClick={() => {
                void revealConfig().catch((err) => setError(String(err)));
              }}><FolderOpen className="size-3.5" /></Button>
          } />
          </Rows>
        </div>
    </Card>
  );
}
