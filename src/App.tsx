import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ArrowDownToLine, Loader2 } from "lucide-react";
import { TitleBar, type Tab } from "./components/TitleBar";
import { ServerStatus } from "./components/ServerStatus";
import { CoreLog } from "./components/CoreLog";
import type { LogEvent } from "./lib/coreLog";
import { createLogStore } from "./lib/logStore";
import { Button } from "./components/ui/button";
import { Switch } from "./components/ui/switch";
import { Card } from "./components/ui/card";
import {
  checkIp,
  connect,
  disconnect,
  restartCore,
  cleanCore,
  restartApp,
  type MaintenanceAction,
  getStatus,
  initialize,
  saveSettings,
  setLanguage,
  type Settings,
  type Status,
} from "./lib/ipc";
import { checkForUpdate, type UpdateAvailable } from "./lib/updateCheck";
import { toLang, useI18n } from "./i18n";

type Busy = "idle" | "connecting" | "disconnecting";

const SettingsPanel = lazy(() => import("./components/SettingsPanel"));
const SshPanel = lazy(() => import("./components/SshPanel"));
const RulesDialog = lazy(() =>
  import("./components/RulesDialog").then((module) => ({ default: module.RulesDialog })),
);

export function App() {
  const { t, lang, setLang } = useI18n();
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState<Busy>("idle");
  const [error, setError] = useState<string | null>(null);
  const [logStore] = useState(createLogStore);
  const [tab, setTab] = useState<Tab>("home");
  const [update, setUpdate] = useState<UpdateAvailable | null>(null);
  const [installing, setInstalling] = useState(false);
  const [ips, setIps] = useState<{ direct?: string; server?: string }>({});
  const [probing, setProbing] = useState(false);
  const [rulesOpen, setRulesOpen] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void initialize().then(({ status: s, error: startupError }) => {
      if (cancelled) return;
      setStatus(s);
      setError(startupError);
      logStore.reset(s.log);
      setLang(toLang(s.settings.language));
    }).catch((err) => {
      if (!cancelled) setError(String(err));
    });
    void checkForUpdate().then((available) => {
      if (!cancelled) setUpdate(available);
    });
    return () => { cancelled = true; };
  }, [setLang, logStore]);

  useEffect(() => {
    const unlisten = Promise.all([
      listen<LogEvent>("core-log", (event) =>
        logStore.append(event.payload),
      ),
      listen<string>("core-exit", () => void getStatus().then(setStatus)),
      listen("connection-changed", () => void getStatus().then(setStatus)),
    ]);
    return () => {
      void unlisten.then((fns) => fns.forEach((fn) => fn()));
    };
  }, [logStore]);

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

  async function maintainCore(action: MaintenanceAction) {
    setError(null);
    setBusy(action === "clean" ? "disconnecting" : "connecting");
    setIps({});
    try {
      if (action === "app") await restartApp();
      else setStatus(action === "clean" ? await cleanCore() : await restartCore());
    } catch (err) {
      setError(String(err));
      setStatus(await getStatus());
    } finally {
      setBusy("idle");
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
      await Promise.all([
        checkIp(false).catch(() => "—").then((direct) => setIps((prev) => ({ ...prev, direct }))),
        checkIp(true).catch(() => "—").then((server) => setIps((prev) => ({ ...prev, server }))),
      ]);
    } finally {
      setProbing(false);
    }
  }

  if (!status) {
    return (
      <div className="flex h-full flex-col">
        <TitleBar tab={tab} onTabChange={setTab} />
        <div className="flex flex-1 items-center justify-center text-xs text-fg-muted">
          {error ? (
            <span role="alert">{error}</span>
          ) : (
            <><Loader2 className="mr-2 h-3.5 w-3.5 animate-spin" />{t("app.loading")}</>
          )}
        </div>
      </div>
    );
  }

  const { settings, running, system_proxy_active, tun_active } = status;
  const connectionRequested = running || Boolean(status.connection_requested);
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
                  working || (connectionRequested && !running) ? "bg-warn pulse-dot" : running ? "bg-ok" : "bg-fg-muted/40",
                ].join(" ")}
              />
              <div className="min-w-0 flex-1">
                <p className="text-sm text-fg" title={running ? routedSummary : t("status.allDirect")}>
                  {working
                    ? busy === "connecting"
                      ? t("status.connecting")
                      : t("status.disconnecting")
                    : connectionRequested && !running
                      ? t("recovery.recovering")
                    : running
                      ? tun_active
                        ? t("status.onTun")
                        : t("status.on")
                      : t("status.off")}
                </p>
              </div>
              <Switch
                checked={connectionRequested}
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
            onEdit={() => setTab("settings")}
          />

          <CoreLog store={logStore} />
        </div>
      ) : tab === "ssh" ? (
        <div className="thin-scroll min-h-0 flex-1 space-y-3 overflow-y-auto px-4 pb-4">
          <Suspense fallback={<p className="text-xs text-fg-muted">{t("app.loading")}</p>}>
            <SshPanel />
          </Suspense>
        </div>
      ) : (
        // A block container, not a flex column: flex children shrink below their
        // content height in a scroll container and spill over each other.
        <div className="thin-scroll min-h-0 flex-1 space-y-3 overflow-y-auto px-4 pb-4">
          {updateBanner}
          {errorBanner}

          <Suspense fallback={<p className="text-xs text-fg-muted">{t("app.loading")}</p>}>
            <SettingsPanel
              status={status}
              working={working}
              persist={persist}
              switchTun={switchTun}
              changeLanguage={changeLanguage}
              onEditFile={() => setRulesOpen(true)}
              ips={ips}
              probing={probing}
              probe={probe}
              maintainCore={maintainCore}
            />
          </Suspense>
        </div>
      )}

      {rulesOpen ? (
        <Suspense fallback={null}>
          <RulesDialog
            open={rulesOpen}
            onOpenChange={setRulesOpen}
            onSaved={(next) => {
              setStatus(next);
              setError(null);
            }}
          />
        </Suspense>
      ) : null}
    </div>
  );
}
