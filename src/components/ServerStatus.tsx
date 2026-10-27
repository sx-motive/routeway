import { useState } from "react";
import { Loader2, RefreshCw, Server as ServerIcon } from "lucide-react";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
import { LatencyChart } from "./LatencyChart";
import { useI18n } from "../i18n";
import type { Server } from "../lib/vless";
import { checkHealth } from "../lib/ipc";
import { PROBE_WINDOW, summarize, useServerHealth, type Sample } from "../lib/useServerHealth";

const SLOW_MS = 1200;

type State = "checking" | "ok" | "slow" | "down";

const DOT: Record<State, string> = {
  checking: "bg-fg-muted/40 pulse-dot",
  ok: "bg-ok",
  slow: "bg-warn",
  down: "bg-danger",
};

function HealthMetric({ samples, active }: { samples: Sample[]; active: boolean }) {
  const { t } = useI18n();
  const [checking, setChecking] = useState(false);
  const { last, lossPct } = summarize(samples);

  const state: State = checking || !last
    ? "checking"
    : last.ms === null
      ? "down"
      : last.ms > SLOW_MS
        ? "slow"
        : "ok";

  const time = (sample: Sample) => new Date(sample.at).toLocaleTimeString();
  const describe = (sample: Sample) =>
    sample.ms === null
      ? `${t("health.down")} · ${sample.error ?? ""} · ${time(sample)}`
      : `${t("health.ms", { ms: sample.ms })} · ${time(sample)}`;

  const checkNow = async () => {
    setChecking(true);
    try {
      await checkHealth();
    } catch (err) {
      console.warn("[health] check failed", err);
    } finally {
      setChecking(false);
    }
  };

  const stateLabel = t(`health.${state}`);
  return (
    <div className="border-t border-border px-4 py-3">
      <div className="flex items-center justify-between gap-3">
        <span className="flex min-w-0 items-center gap-2 text-xs text-fg-muted">
          <span className={`size-2 shrink-0 ${active ? DOT[state] : "bg-fg-muted/30"}`}
            title={active ? stateLabel : undefined} aria-label={active ? stateLabel : undefined} />
          {t("health.modeTunnel")}
        </span>
        <span className="flex shrink-0 items-center gap-2 font-mono text-xs text-fg-muted">
          <span className="text-sm text-fg">{active && last?.ms != null ? t("health.ms", { ms: last.ms }) : "—"}</span>
          {active && last ? <span className={lossPct > 0 ? "text-warn" : undefined}>
            {t("health.failures", { pct: lossPct })}
          </span> : null}
          {active ? (
            <Button variant="ghost" size="icon" className="-mr-2 h-7 w-7" disabled={checking}
              onClick={() => void checkNow()} title={t("health.checkNow")} aria-label={t("health.checkNow")}>
              {checking ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
            </Button>
          ) : null}
        </span>
      </div>
      {active
        ? <div className="mt-2.5"><LatencyChart samples={samples} slots={PROBE_WINDOW} describe={describe} /></div>
        : <p className="mt-2 text-xs text-fg-muted">{t("health.afterConnect")}</p>}
    </div>
  );
}

export function ServerStatus({ server, onEdit }: {
  server: Server | null;
  onEdit: () => void;
}) {
  const { t } = useI18n();
  const health = useServerHealth();
  return (
    <Card className="shrink-0">
      <div className="flex min-h-12 items-center gap-3 px-4 py-2">
        <ServerIcon className="h-3.5 w-3.5 shrink-0 text-fg-muted" />
        {server ? (
          <div className="flex min-w-0 flex-1 items-baseline gap-2">
            <span className="truncate text-sm text-fg">{server.label || server.address}</span>
            <span className="truncate font-mono text-xs text-fg-muted">
              {server.address}:{server.port}
            </span>
          </div>
        ) : (
          <span className="flex-1 text-xs text-fg-muted">{t("server.none")}</span>
        )}
        <Button variant="ghost" size="sm" className="-mr-2 h-7" onClick={onEdit}>
          {server ? t("server.edit") : t("server.setup")}
        </Button>
      </div>

      {server ? <HealthMetric samples={health.tunnel} active={health.tunnel_active} /> : null}
    </Card>
  );
}
