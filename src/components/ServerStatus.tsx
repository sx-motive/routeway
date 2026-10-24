import { Server as ServerIcon } from "lucide-react";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
import { LatencyChart } from "./LatencyChart";
import { useI18n } from "../i18n";
import type { Server } from "../lib/vless";
import { PROBE_WINDOW, summarize, useServerHealth, type Sample } from "../lib/useServerHealth";
import type { Health } from "../lib/ipc";

const SLOW_MS = { tcp: 300, tunnel: 1200 } as const;

type State = "checking" | "ok" | "slow" | "down";

const DOT: Record<State, string> = {
  checking: "bg-fg-muted/40 pulse-dot",
  ok: "bg-ok",
  slow: "bg-warn",
  down: "bg-danger",
};

function HealthMetric({ mode, samples, active, available = true, recovery }: {
  mode: "tcp" | "tunnel";
  samples: Sample[];
  active: boolean;
  available?: boolean;
  recovery?: Health["recovery"];
}) {
  const { t } = useI18n();
  const recoveryActive = recovery && recovery !== "idle" && recovery !== "healthy";
  const { last, lossPct } = summarize(samples);

  const state: State = !last || recovery === "checking" || recovery === "recovering"
    ? "checking"
    : last.ms === null
      ? "down"
      : last.ms > SLOW_MS[mode]
        ? "slow"
        : "ok";

  const time = (sample: Sample) => new Date(sample.at).toLocaleTimeString();
  const describe = (sample: Sample) =>
    sample.ms === null
      ? `${t("health.down")} · ${sample.error ?? ""} · ${time(sample)}`
      : `${t("health.ms", { ms: sample.ms })} · ${time(sample)}`;

  const enabled = active && available;
  const stateLabel = recoveryActive ? t(`recovery.${recovery}`) : t(`health.${state}`);
  return (
    <div className="border-t border-border px-4 py-3">
      <div className="flex items-center justify-between gap-3">
        <span className="flex min-w-0 items-center gap-2 text-xs text-fg-muted">
          <span className={`size-2 shrink-0 ${enabled ? DOT[state] : "bg-fg-muted/30"}`}
            title={enabled ? stateLabel : undefined} aria-label={enabled ? stateLabel : undefined} />
          {mode === "tcp" ? t("health.modeTcp") : t("health.modeTunnel")}
        </span>
        <span className="flex shrink-0 items-baseline gap-2 font-mono text-xs text-fg-muted">
          <span className="text-sm text-fg">{enabled && last?.ms != null ? t("health.ms", { ms: last.ms }) : "—"}</span>
          {enabled && last ? <span className={lossPct > 0 ? "text-warn" : undefined}>
            {t("health.failures", { pct: lossPct })}
          </span> : null}
        </span>
      </div>
      {enabled ? <>
        <div className="mt-2.5"><LatencyChart samples={samples} slots={PROBE_WINDOW} describe={describe} /></div>
        {recoveryActive ? <p role="status" className="mt-2 text-xs text-warn">{stateLabel}</p> : null}
      </> : <p className="mt-2 text-xs text-fg-muted">
        {t(available ? "health.afterConnect" : "health.tcpUnavailableTun")}
      </p>}
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

      {server ? (
        <>
          <HealthMetric mode="tcp" samples={health.tcp} active available={health.tcp_available} />
          <HealthMetric mode="tunnel" samples={health.tunnel} active={health.tunnel_active} recovery={health.recovery} />
        </>
      ) : null}
    </Card>
  );
}
