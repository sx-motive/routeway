import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowDownToLine,
  Loader2,
  RefreshCw,
  ShieldCheck,
} from "lucide-react";
import { TitleBar, type Tab } from "./components/TitleBar";
import { ServerCard } from "./components/ServerCard";
import { DomainList } from "./components/DomainList";
import { DirectList } from "./components/DirectList";
import { ProcessList } from "./components/ProcessList";
import { RulesDialog } from "./components/RulesDialog";
import { ServerStatus } from "./components/ServerStatus";
import { useServerHealth } from "./lib/useServerHealth";
import { CoreLog } from "./components/CoreLog";
import { stampLocalTime, type LogEvent } from "./lib/coreLog";
import { Button } from "./components/ui/button";
import { Switch } from "./components/ui/switch";
import { Card, Row, Rows } from "./components/ui/card";
import { NumericInput } from "./components/ui/numeric-input";
import {
  checkIp,
  connect,
  disconnect,
  getStatus,
  saveSettings,
  setLanguage,
  type Settings,
  type Status,
} from "./lib/ipc";
import type { Server } from "./lib/vless";
import { checkForUpdate, type UpdateAvailable } from "./lib/updateCheck";
import { LANGS, toLang, useI18n } from "./i18n";

type Busy = "idle" | "connecting" | "disconnecting";

export function App() {
  const { t, lang, setLang } = useI18n();
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState<Busy>("idle");
  const [error, setError] = useState<string | null>(null);
  const [log, setLog] = useState<LogEvent[]>([]);
  const [tab, setTab] = useState<Tab>("home");
  const health = useServerHealth();
  const [update, setUpdate] = useState<UpdateAvailable | null>(null);
  const [installing, setInstalling] = useState(false);
  const [ips, setIps] = useState<{ direct?: string; server?: string }>({});
  const [probing, setProbing] = useState(false);
  const [rulesOpen, setRulesOpen] = useState(false);

  useEffect(() => {
    void getStatus().then((s) => {
      setStatus(s);
      setLog(s.log.map(stampLocalTime));
      setLang(toLang(s.settings.language));
    });
    void checkForUpdate().then(setUpdate);
  }, [setLang]);

  useEffect(() => {
    const unlisten = Promise.all([
      listen<LogEvent>("core-log", (event) =>
        setLog((prev) => [...prev.slice(-299), stampLocalTime(event.payload)]),
      ),
      listen<string>("core-exit", () => void getStatus().then(setStatus)),
    ]);
    return () => {
      void unlisten.then((fns) => fns.forEach((fn) => fn()));
    };
  }, []);

  const persist = useCallback(async (next: Settings) => {
    setStatus((prev) => (prev ? { ...prev, settings: next } : prev));
    try {
      setStatus(await saveSettings(next));
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  async function toggle(on: boolean) {
    setError(null);
    setBusy(on ? "connecting" : "disconnecting");
    try {
      setStatus(on ? await connect() : await disconnect());
      if (!on) setIps({});
    } catch (err) {
      setError(String(err));
      void getStatus().then(setStatus);
    } finally {
      setBusy("idle");
    }
  }

  async function switchTun(next: Settings) {
    if (!running) {
      await persist(next);
      return;
    }
    setBusy("connecting");
    try {
      await persist(next);
    } finally {
      setBusy("idle");
      void getStatus().then(setStatus);
    }
  }

  async function changeLanguage(next: typeof lang) {
    setLang(next);
    try {
      setStatus(await setLanguage(next));
    } catch (err) {
      setError(String(err));
    }
  }

  async function probe() {
    setProbing(true);
    setIps({});
    try {
      const direct = await checkIp(false).catch(() => "—");
      setIps({ direct });
      const server = await checkIp(true).catch(() => "—");
      setIps({ direct, server });
    } finally {
      setProbing(false);
    }
  }

  if (!status) {
    return (
      <div className="flex h-full flex-col">
        <TitleBar tab={tab} onTabChange={setTab} />
        <div className="flex flex-1 items-center justify-center text-xs text-fg-muted">
          <Loader2 className="mr-2 h-3.5 w-3.5 animate-spin" />
          {t("app.loading")}
        </div>
      </div>
    );
  }

  const { settings, running, system_proxy_active, tun_active } = status;
  const working = busy !== "idle";

  const routed = [
    settings.processes.length > 0
      ? t("count.apps", { n: settings.processes.length })
      : null,
    settings.domains.length > 0
      ? t("count.domains", { n: settings.domains.length })
      : null,
  ].filter((part): part is string => part !== null);
  const routedSummary =
    routed.length === 0
      ? t("status.nothingRouted")
      : t("status.routed", {
          items: routed.length === 2 ? t("status.join", { a: routed[0], b: routed[1] }) : routed[0],
          server: settings.server?.label ?? t("status.server"),
        });

  const updateBanner = update ? (
    <div className="flex shrink-0 items-center justify-between border border-accent/40 bg-accent/5 px-4 py-2">
      <span className="text-xs text-fg">
        {t("update.available", { version: update.version })}
        {update.notes ? <span className="text-fg-muted"> · {update.notes}</span> : null}
      </span>
      <Button
        size="sm"
        disabled={installing}
        onClick={() => {
          setInstalling(true);
          void update.install().catch((err) => {
            setError(String(err));
            setInstalling(false);
          });
        }}
      >
        {installing ? (
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
        ) : (
          <ArrowDownToLine className="h-3.5 w-3.5" />
        )}
        {t("update.install")}
      </Button>
    </div>
  ) : null;

  const errorBanner = error ? (
    <p className="shrink-0 border border-danger/40 bg-danger/5 px-4 py-2.5 text-xs text-danger">
      {error}
    </p>
  ) : null;

  return (
    <div className="flex h-full flex-col">
      <TitleBar tab={tab} onTabChange={setTab} />

      {tab === "home" ? (
        <div className="thin-scroll flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 pb-4">
          {updateBanner}

          <Card className="shrink-0">
            <div className="flex min-h-16 items-center gap-3 px-4 py-3">
              <span
                className={[
                  "h-2 w-2 shrink-0",
                  working ? "bg-warn pulse-dot" : running ? "bg-ok" : "bg-fg-muted/40",
                ].join(" ")}
              />
              <div className="min-w-0 flex-1">
                <p className="text-sm text-fg">
                  {working
                    ? busy === "connecting"
                      ? t("status.connecting")
                      : t("status.disconnecting")
                    : running
                      ? tun_active
                        ? t("status.onTun")
                        : t("status.on")
                      : t("status.off")}
                </p>
                <p className="truncate text-xs text-fg-muted">
                  {running ? routedSummary : t("status.allDirect")}
                </p>
              </div>
              <Switch
                checked={running}
                disabled={working || !settings.server}
                onCheckedChange={(on) => void toggle(on)}
                aria-label={t("status.toggle")}
              />
            </div>
            {running && !tun_active && settings.system_proxy && !system_proxy_active ? (
              <p className="border-t border-border px-4 py-2.5 text-xs text-warn">
                {t("warn.proxyNotSet")}
              </p>
            ) : null}
            {running && !tun_active && !settings.system_proxy ? (
              <p className="border-t border-border px-4 py-2.5 text-xs text-fg-muted">
                {t("warn.proxyOff", { port: settings.listen_port })}
              </p>
            ) : null}
          </Card>

          {errorBanner}

          <ServerStatus
            server={settings.server}
            health={health}
            onEdit={() => setTab("settings")}
          />

          <CoreLog events={log} onClear={() => setLog([])} />
        </div>
      ) : (
        // A block container, not a flex column: flex children shrink below their
        // content height in a scroll container and spill over each other.
        <div className="thin-scroll min-h-0 flex-1 space-y-3 overflow-y-auto px-4 pb-4">
          {updateBanner}
          {errorBanner}

          <Card title={t("settings.general")}>
            <Rows>
              <Row
                interactive={false}
                label={t("settings.language")}
                control={
                  <div className="flex h-8 border border-border bg-field">
                    {LANGS.map(({ id, label }) => (
                      <button
                        key={id}
                        type="button"
                        aria-pressed={lang === id}
                        onClick={() => void changeLanguage(id)}
                        className={[
                          "px-3 text-xs transition-colors",
                          lang === id
                            ? "bg-bg-elevated text-fg"
                            : "text-fg-muted hover:text-fg",
                        ].join(" ")}
                      >
                        {label}
                      </button>
                    ))}
                  </div>
                }
              />
              <Row
                label={t("settings.autostart")}
                control={
                  <Switch
                    checked={settings.autostart}
                    onCheckedChange={(on) => void persist({ ...settings, autostart: on })}
                    aria-label={t("settings.autostart")}
                  />
                }
              />
            </Rows>
          </Card>

          <ServerCard
            server={settings.server}
            onChange={(server: Server) => void persist({ ...settings, server })}
          />

          <Card title={t("settings.routing")}>
            <Rows>
              <Row
                label={t("settings.tun")}
                hint={t("settings.tunHint")}
                control={
                  <Switch
                    checked={settings.tun}
                    disabled={working}
                    onCheckedChange={(on) => void switchTun({ ...settings, tun: on })}
                    aria-label={t("settings.tun")}
                  />
                }
              />
              <Row
                label={t("settings.proxy")}
                hint={
                  settings.tun
                    ? t("settings.proxyTunHint")
                    : t("settings.proxyHint", { port: settings.listen_port })
                }
                control={
                  <Switch
                    checked={settings.system_proxy && !settings.tun}
                    disabled={settings.tun}
                    onCheckedChange={(on) => void persist({ ...settings, system_proxy: on })}
                    aria-label={t("settings.proxy")}
                  />
                }
              />
              <Row
                label={t("settings.port")}
                hint={t("settings.portHint", {
                  port: settings.listen_port,
                  probe: settings.listen_port + 1,
                })}
                control={
                  <NumericInput
                    min={1025}
                    max={65534}
                    value={settings.listen_port}
                    disabled={running}
                    title={running ? t("settings.portLocked") : undefined}
                    onValueChange={(port) => void persist({ ...settings, listen_port: port })}
                    className="h-8 w-24 px-2 text-right font-mono text-xs"
                  />
                }
              />
            </Rows>
          </Card>

          <ProcessList
            processes={settings.processes}
            onChange={(processes) => void persist({ ...settings, processes })}
            onEditFile={() => setRulesOpen(true)}
          />

          <DomainList
            domains={settings.domains}
            onChange={(domains) => void persist({ ...settings, domains })}
            onEditFile={() => setRulesOpen(true)}
          />

          <DirectList
            domains={settings.direct}
            onChange={(direct) => void persist({ ...settings, direct })}
            onEditFile={() => setRulesOpen(true)}
          />

          <Card
            title={t("check.title")}
            aside={
              <Button
                variant="ghost"
                size="sm"
                className="-mr-2 h-7"
                disabled={!running || probing}
                onClick={probe}
              >
                {probing ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="h-3.5 w-3.5" />
                )}
                {t("check.run")}
              </Button>
            }
          >
            <div className="grid grid-cols-2 divide-x divide-border">
              <div className="px-4 py-3">
                <p className="text-xs text-fg-muted">{t("check.direct")}</p>
                <p className="mt-0.5 font-mono text-sm text-fg">{ips.direct ?? "—"}</p>
              </div>
              <div className="px-4 py-3">
                <p className="text-xs text-fg-muted">{t("check.server")}</p>
                <p className="mt-0.5 font-mono text-sm text-fg">
                  {ips.server ? (
                    <span className="inline-flex items-center gap-1.5">
                      {ips.server}
                      {ips.direct && ips.server !== "—" && ips.server !== ips.direct ? (
                        <ShieldCheck className="h-3.5 w-3.5 text-ok" />
                      ) : null}
                    </span>
                  ) : (
                    "—"
                  )}
                </p>
              </div>
            </div>
          </Card>
        </div>
      )}

      <RulesDialog
        open={rulesOpen}
        onOpenChange={setRulesOpen}
        onSaved={(next) => {
          setStatus(next);
          setError(null);
        }}
      />
    </div>
  );
}
