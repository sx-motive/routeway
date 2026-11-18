import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { ChevronRight, CircleAlert, CircleCheck, Info, TriangleAlert } from "lucide-react";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
import { useI18n } from "../i18n";
import { groupLog, reasonKey, splitTarget, type LogEvent, type LogRow } from "../lib/coreLog";
import type { LogStore } from "../lib/logStore";

function LevelIcon({ event }: { event: LogEvent }) {
  const className = "mt-0.5 h-3.5 w-3.5 shrink-0";
  if (event.level === "error") return <CircleAlert className={`${className} text-danger`} />;
  if (event.level === "warn") return <TriangleAlert className={`${className} text-warn`} />;
  if (event.kind === "started") return <CircleCheck className={`${className} text-ok`} />;
  return <Info className={`${className} text-fg-muted`} />;
}

function Row({ row }: { row: LogRow }) {
  const { t } = useI18n();
  const { event } = row;
  const { host } = splitTarget(event.target);
  const shownHost = event.domain ?? host ?? "?";
  const reason = reasonKey(event.reason);

  let title: string;
  switch (event.kind) {
    case "started":
      title = event.detail ? t("log.startedIn", { time: event.detail }) : t("log.started");
      break;
    case "stopped":
      title = event.raw;
      break;
    case "listening":
      title = t("log.listening", { target: event.target ?? "" });
      break;
    case "interface":
      title = t("log.interface", { name: event.detail ?? "" });
      break;
    case "connection_failed":
      title = event.app
        ? t("log.failedApp", { app: event.app, host: shownHost })
        : t("log.failed", { host: shownHost });
      break;
    default:
      title = event.detail ?? event.raw;
  }

  const details: string[] = [];
  if (event.kind === "connection_failed" || event.kind === "other") {
    if (event.route) details.push(event.route === "proxy" ? t("log.viaProxy") : t("log.direct"));
    if (event.domain && host && host !== event.domain) details.push(host);
    if (row.ports.length) details.push(t("log.ports", { ports: row.ports.join(", ") }));
    if (event.reason && event.kind === "connection_failed") {
      details.push(reason ? t(`log.reason.${reason}`) : event.reason);
    }
  }

  const blockedHint =
    event.kind === "connection_failed" &&
    event.route === "direct" &&
    (reason === "timeout" || reason === "reset")
      ? t("log.hintBlocked", { what: event.app ?? shownHost })
      : null;

  return (
    <li className="flex gap-3 border-b border-border/60 px-4 py-2 last:border-b-0" title={event.raw}>
      <LevelIcon event={event} />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="min-w-0 truncate text-xs text-fg">{title}</span>
          {row.count > 1 ? (
            <span className="shrink-0 rounded-full bg-bg-elevated px-1.5 font-mono text-[10px] text-fg-muted">
              ×{row.count}
            </span>
          ) : null}
        </div>
        {details.length ? (
          <p className="mt-0.5 truncate text-[11px] text-fg-muted">{details.join(" · ")}</p>
        ) : null}
        {blockedHint ? <p className="mt-0.5 text-[11px] text-warn">{blockedHint}</p> : null}
      </div>
      <span className="shrink-0 font-mono text-[11px] text-fg-muted">{row.lastTime}</span>
    </li>
  );
}

const OPEN_KEY = "routeway.logOpen";

function readOpen() {
  try {
    return localStorage.getItem(OPEN_KEY) === "1";
  } catch {
    return false;
  }
}

export function CoreLog({ store }: { store: LogStore }) {
  const events = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const { t } = useI18n();
  const [raw, setRaw] = useState(false);
  const [open, setOpen] = useState(readOpen);
  const scrollRef = useRef<HTMLDivElement>(null);
  const rows = useMemo(() => groupLog(events), [events]);
  const problems = useMemo(
    () => events.filter((event) => event.level === "error" || event.level === "warn").length,
    [events],
  );

  function toggleOpen() {
    setOpen((value) => {
      try {
        localStorage.setItem(OPEN_KEY, value ? "0" : "1");
      } catch {
        // The log simply opens collapsed next time.
      }
      return !value;
    });
  }

  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [events, raw, open]);

  return (
    <Card className={open ? "flex min-h-40 flex-1 flex-col" : "shrink-0"}>
      <header className="flex h-11 shrink-0 items-center gap-2 pr-2">
        <button
          type="button"
          aria-expanded={open}
          onClick={toggleOpen}
          className="flex h-full min-w-0 flex-1 items-center gap-2 pl-4 text-left text-xs font-medium text-fg-muted transition-colors outline-none hover:text-fg focus-visible:text-fg"
        >
          <ChevronRight className={`size-3.5 shrink-0 transition-transform duration-200 ${open ? "rotate-90" : ""}`} />
          <span className="truncate">{t("log.title")}</span>
          {problems > 0 ? (
            <span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-warn/10 px-1.5 font-mono text-[10px] text-warn">
              <TriangleAlert className="size-3" />
              {problems}
            </span>
          ) : null}
        </button>
        {open ? (
          <span className="flex shrink-0 items-center gap-1">
            <Button variant="ghost" size="sm" className="h-7" aria-pressed={raw} onClick={() => setRaw((v) => !v)}>
              {raw ? t("log.readable") : t("log.raw")}
            </Button>
            <Button variant="ghost" size="sm" className="h-7" disabled={!events.length} onClick={store.clear}>
              {t("log.clear")}
            </Button>
          </span>
        ) : null}
      </header>
      {open ? (
      <div ref={scrollRef} className="thin-scroll min-h-0 flex-1 overflow-y-auto border-t border-border">
        {events.length === 0 ? (
          <p className="px-4 py-3 text-xs text-fg-muted">{t("log.empty")}</p>
        ) : raw ? (
          <div className="px-4 py-2">
            {events.map((event, index) => (
              <p key={index} className="font-mono text-[11px] leading-relaxed break-all text-fg-muted">
                {event.raw}
              </p>
            ))}
          </div>
        ) : (
          <ul>
            {rows.map((row) => (
              <Row key={row.key} row={row} />
            ))}
          </ul>
        )}
      </div>
      ) : null}
    </Card>
  );
}
