import { Server as ServerIcon } from "lucide-react";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
import { LatencyBars } from "./LatencyBars";
import { useI18n } from "../i18n";
import type { Server } from "../lib/vless";
import { PROBE_WINDOW, summarize, type Health, type Sample } from "../lib/useServerHealth";

const SLOW_MS = { tcp: 300, tunnel: 1200 } as const;

type State = "checking" | "ok" | "slow" | "down";

const DOT: Record<State, string> = {
  checking: "bg-fg-muted/40 pulse-dot",
  ok: "bg-ok",
  slow: "bg-warn",
  down: "bg-danger",
};

export function ServerStatus({
  server,
  health,
  onEdit,
}: {
  server: Server | null;
  health: Health;
  onEdit: () => void;
}) {
  const { t } = useI18n();
  const { mode, samples } = health;
  const { last, avg, lossPct } = summarize(samples);

  const state: State = !last
    ? "checking"
    : last.ms === null
      ? "down"
      : last.ms > SLOW_MS[mode ?? "tcp"]
        ? "slow"
        : "ok";

  const time = (sample: Sample) => new Date(sample.at).toLocaleTimeString();
  const describe = (sample: Sample) =>
    sample.ms === null
      ? `${t("health.down")} · ${sample.error ?? ""} · ${time(sample)}`
      : `${t("health.ms", { ms: sample.ms })} · ${time(sample)}`;

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
        <div className="border-t border-border px-4 py-3">
          <div className="flex items-center justify-between gap-3">
            <span className="flex min-w-0 items-center gap-2 text-xs">
              <span className={`h-2 w-2 shrink-0 ${DOT[state]}`} />
              <span className="text-fg">{t(`health.${state}`)}</span>
              {mode ? (
                <span className="truncate text-fg-muted">
                  · {mode === "tunnel" ? t("health.modeTunnel") : t("health.modeTcp")}
                </span>
              ) : null}
            </span>
            {last ? (
              <span className="flex shrink-0 items-baseline gap-2 font-mono text-xs text-fg-muted">
                <span className="text-sm text-fg">
                  {last.ms === null ? "—" : t("health.ms", { ms: last.ms })}
                </span>
                {avg !== null ? <span>{t("health.avg", { ms: avg })}</span> : null}
                <span className={lossPct > 0 ? "text-warn" : undefined}>
                  {t("health.loss", { pct: lossPct })}
                </span>
              </span>
            ) : null}
          </div>
          <div className="mt-2.5">
            <LatencyBars samples={samples} slots={PROBE_WINDOW} describe={describe} />
          </div>
        </div>
      ) : null}
    </Card>
  );
}
