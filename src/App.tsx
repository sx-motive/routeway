import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ArrowDownToLine, Loader2, RefreshCw, ShieldCheck, Terminal } from "lucide-react";
import { TitleBar } from "./components/TitleBar";
import { ServerCard } from "./components/ServerCard";
import { DomainList } from "./components/DomainList";
import { Button } from "./components/ui/button";
import { Switch } from "./components/ui/switch";
import {
  checkIp,
  connect,
  disconnect,
  getStatus,
  saveSettings,
  type Settings,
  type Status,
} from "./lib/ipc";
import type { Server } from "./lib/vless";
import { checkForUpdate, type UpdateAvailable } from "./lib/updateCheck";

type Busy = "idle" | "connecting" | "disconnecting";

export function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState<Busy>("idle");
  const [error, setError] = useState<string | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [showLog, setShowLog] = useState(false);
  const [update, setUpdate] = useState<UpdateAvailable | null>(null);
  const [installing, setInstalling] = useState(false);
  const [ips, setIps] = useState<{ direct?: string; server?: string }>({});
  const [probing, setProbing] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void getStatus().then((s) => {
      setStatus(s);
      setLog(s.log);
    });
    void checkForUpdate().then(setUpdate);
  }, []);

  useEffect(() => {
    const unlisten = Promise.all([
      listen<string>("core-log", (event) => setLog((prev) => [...prev.slice(-299), event.payload])),
      listen<string>("core-exit", (event) => {
        setLog((prev) => [...prev.slice(-299), event.payload]);
        void getStatus().then(setStatus);
      }),
    ]);
    return () => {
      void unlisten.then((fns) => fns.forEach((fn) => fn()));
    };
  }, []);

  useEffect(() => {
    if (showLog && logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [log, showLog]);

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
        <TitleBar />
        <div className="flex flex-1 items-center justify-center text-xs text-fg-muted">
          <Loader2 className="mr-2 h-3.5 w-3.5 animate-spin" />
          Загрузка…
        </div>
      </div>
    );
  }

  const { settings, running, system_proxy_active } = status;
  const working = busy !== "idle";
  const domainCount = settings.domains.length;

  return (
    <div className="flex h-full flex-col">
      <TitleBar />

      <div className="thin-scroll flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 pb-4">
        {update ? (
          <div className="flex items-center justify-between border border-accent/40 bg-accent/5 px-3 py-2">
            <span className="text-xs text-fg">
              Доступна версия {update.version}
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
              Обновить
            </Button>
          </div>
        ) : null}

        <section className="border border-border">
          <div className="flex items-center gap-3 px-3 py-3">
            <span
              className={[
                "h-2 w-2 shrink-0",
                working
                  ? "bg-warn pulse-dot"
                  : running
                    ? "bg-ok"
                    : "bg-fg-muted/40",
              ].join(" ")}
            />
            <div className="min-w-0 flex-1">
              <p className="text-sm text-fg">
                {working
                  ? busy === "connecting"
                    ? "Подключаюсь…"
                    : "Отключаюсь…"
                  : running
                    ? "Работает"
                    : "Выключено"}
              </p>
              <p className="truncate text-xs text-fg-muted">
                {running
                  ? `${domainCount === 0 ? "ни один домен не" : `${domainCount} домен(ов)`} через ${settings.server?.label ?? "сервер"} · остальное напрямую`
                  : "весь трафик идёт напрямую"}
              </p>
            </div>
            <Switch
              checked={running}
              disabled={working || !settings.server}
              onCheckedChange={(on) => void toggle(on)}
              aria-label="Включить маршрутизацию"
            />
          </div>

          <div className="flex flex-col gap-2 border-t border-border px-3 py-2.5">
            <label className="flex items-center justify-between gap-3">
              <span className="text-xs text-fg-muted">
                Системный прокси
                <span className="text-fg-muted/60">
                  {" "}
                  · направить трафик системы на 127.0.0.1:{settings.listen_port}
                </span>
              </span>
              <Switch
                checked={settings.system_proxy}
                onCheckedChange={(on) => void persist({ ...settings, system_proxy: on })}
                aria-label="Системный прокси"
              />
            </label>
            <label className="flex items-center justify-between gap-3">
              <span className="text-xs text-fg-muted">Запускать при входе в систему</span>
              <Switch
                checked={settings.autostart}
                onCheckedChange={(on) => void persist({ ...settings, autostart: on })}
                aria-label="Автозапуск"
              />
            </label>
            {running && settings.system_proxy && !system_proxy_active ? (
              <p className="text-xs text-warn">
                Ядро работает, но системный прокси не выставлен — трафик идёт напрямую.
              </p>
            ) : null}
          </div>
        </section>

        {error ? (
          <p className="border border-danger/40 bg-danger/5 px-3 py-2 text-xs text-danger">
            {error}
          </p>
        ) : null}

        <ServerCard
          server={settings.server}
          onChange={(server: Server) => void persist({ ...settings, server })}
        />

        <DomainList
          domains={settings.domains}
          onChange={(domains) => void persist({ ...settings, domains })}
        />

        <section className="border border-border">
          <header className="flex items-center justify-between border-b border-border px-3 py-2">
            <h2 className="text-xs font-medium tracking-wide text-fg-muted uppercase">
              Проверка
            </h2>
            <Button variant="ghost" size="sm" disabled={!running || probing} onClick={probe}>
              {probing ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <RefreshCw className="h-3.5 w-3.5" />
              )}
              Проверить IP
            </Button>
          </header>
          <div className="grid grid-cols-2 divide-x divide-border">
            <div className="px-3 py-3">
              <p className="text-[11px] text-fg-muted/70">Напрямую</p>
              <p className="font-mono text-xs text-fg">{ips.direct ?? "—"}</p>
            </div>
            <div className="px-3 py-3">
              <p className="text-[11px] text-fg-muted/70">Через сервер</p>
              <p className="font-mono text-xs text-fg">
                {ips.server ? (
                  <span className="inline-flex items-center gap-1.5">
                    {ips.server}
                    {ips.direct && ips.server !== "—" && ips.server !== ips.direct ? (
                      <ShieldCheck className="h-3 w-3 text-ok" />
                    ) : null}
                  </span>
                ) : (
                  "—"
                )}
              </p>
            </div>
          </div>
        </section>

        <section className="border border-border">
          <button
            type="button"
            className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-fg-muted hover:bg-bg-elevated"
            onClick={() => setShowLog((v) => !v)}
          >
            <Terminal className="h-3.5 w-3.5" />
            Журнал ядра
            <span className="ml-auto text-fg-muted/50">{showLog ? "скрыть" : "показать"}</span>
          </button>
          {showLog ? (
            <div
              ref={logRef}
              className="thin-scroll max-h-40 overflow-y-auto border-t border-border bg-bg px-3 py-2"
            >
              {log.length === 0 ? (
                <p className="text-xs text-fg-muted/50">пусто</p>
              ) : (
                log.map((line, index) => (
                  <p key={index} className="font-mono text-[11px] leading-relaxed text-fg-muted">
                    {line}
                  </p>
                ))
              )}
            </div>
          ) : null}
        </section>
      </div>
    </div>
  );
}
