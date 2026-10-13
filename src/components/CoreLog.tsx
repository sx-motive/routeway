import { useEffect, useMemo, useRef, useState } from "react";
import { CircleAlert, CircleCheck, Info, Terminal, TriangleAlert } from "lucide-react";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
import { useI18n } from "../i18n";
import { groupLog, reasonKey, splitTarget, type LogEvent, type LogRow } from "../lib/coreLog";

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
            <span className="shrink-0 border border-border px-1 font-mono text-[10px] text-fg-muted">
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

export function CoreLog({ events, onClear }: { events: LogEvent[]; onClear: () => void }) {
  const { t } = useI18n();
  const [raw, setRaw] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const rows = useMemo(() => groupLog(events), [events]);

  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [events, raw]);

  return (
    <Card
      className="flex min-h-40 flex-1 flex-col"
      title={
        <span className="inline-flex items-center gap-2">
          <Terminal className="h-3.5 w-3.5" />
          {t("log.title")}
        </span>
      }
      aside={
        <span className="-mr-2 flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            className="h-7"
            aria-pressed={raw}
            onClick={() => setRaw((v) => !v)}
          >
            {raw ? t("log.readable") : t("log.raw")}
          </Button>
          <Button variant="ghost" size="sm" className="h-7" disabled={!events.length} onClick={onClear}>
            {t("log.clear")}
          </Button>
        </span>
      }
    >
      <div ref={scrollRef} className="thin-scroll min-h-0 flex-1 overflow-y-auto bg-bg">
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
    </Card>
  );
}
