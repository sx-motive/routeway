import { Button } from "./ui/button";
import { Card } from "./ui/card";
import { Switch } from "./ui/switch";
import { useI18n } from "../i18n";
import type { Status } from "../lib/ipc";
import { summarize, useServerHealth } from "../lib/useServerHealth";

const SLOW_MS = 1200;
// WebView2 on Windows has no flag glyphs and prints "🇵🇱" as the letters "PL".
const FLAG = /[\u{1F1E6}-\u{1F1FF}]{2}\s*/gu;

/** Latency of the last tunnel probe, plus the failure rate when there is one. */
function Latency() {
  const { t } = useI18n();
  const health = useServerHealth();
  if (!health.tunnel_active) return null;
  const { last, lossPct } = summarize(health.tunnel);
  if (!last) return null;

  const tone = last.ms === null ? "text-danger" : last.ms > SLOW_MS ? "text-warn" : "text-fg/80";
  const at = new Date(last.at).toLocaleTimeString();
  return (
    <>
      <span aria-hidden className="text-fg-muted/50">·</span>
      <span className={`shrink-0 tabular-nums ${tone}`} title={last.error ? `${last.error} · ${at}` : at}>
        {last.ms === null ? t("health.down") : t("health.ms", { ms: last.ms })}
      </span>
      {lossPct > 0 ? (
        <>
          <span aria-hidden className="text-fg-muted/50">·</span>
          <span className="shrink-0 text-warn">{t("health.failures", { pct: lossPct })}</span>
        </>
      ) : null}
    </>
  );
}

export function ConnectionCard({ status, busy, onToggle, onEditServer }: {
  status: Status;
  busy: "idle" | "connecting" | "disconnecting";
  onToggle: (on: boolean) => void;
  onEditServer: () => void;
}) {
  const { t } = useI18n();
  const { settings, running, system_proxy_active, tun_active } = status;
  const server = settings.server;
  const requested = running || Boolean(status.connection_requested);
  const working = busy !== "idle";
  const pending = working || (requested && !running);

  const serverName = server?.label?.replace(FLAG, "").trim() || server?.address || t("status.server");
  const routed = [
    settings.processes.length > 0 ? t("count.apps", { n: settings.processes.length }) : null,
    settings.domains.length > 0 ? t("count.domains", { n: settings.domains.length }) : null,
  ].filter((part): part is string => part !== null);
  const summary = !running
    ? t("status.allDirect")
    : settings.route_all
      ? t("status.allRouted", { server: serverName })
      : routed.length === 0
        ? t("status.nothingRouted")
        : t("status.routed", {
            items: routed.length === 2 ? t("status.join", { a: routed[0], b: routed[1] }) : routed[0],
            server: serverName,
          });

  const label = working
    ? busy === "connecting" ? t("status.connecting") : t("status.disconnecting")
    : requested && !running
      ? t("recovery.recovering")
      : running
        ? tun_active ? t("status.onTun") : t("status.on")
        : t("status.off");

  // Only a real problem earns a line here; a deliberately disabled proxy does not.
  const warning = running && !tun_active && settings.system_proxy && !system_proxy_active
    ? t("warn.proxyNotSet")
    : null;

  return (
    <Card className="shrink-0">
      <div className="flex items-center gap-3.5 p-4">
        <span
          className={[
            "size-2.5 shrink-0 rounded-full transition-colors",
            pending
              ? "bg-warn pulse-dot"
              : running
                ? "bg-ok shadow-[0_0_10px_2px] shadow-ok/40"
                : "bg-fg-muted/40",
          ].join(" ")}
        />
        <div className="min-w-0 flex-1">
          <p className="text-[15px] leading-5 font-medium text-fg" title={summary}>{label}</p>
          <p className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs leading-4 text-fg-muted">
            {server ? (
              <button type="button" onClick={onEditServer}
                title={`${server.address}:${server.port}`}
                className="min-w-0 truncate rounded transition-colors hover:text-fg focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none">
                {serverName}
              </button>
            ) : (
              <span className="truncate">{t("server.none")}</span>
            )}
            {server && running ? <Latency /> : null}
          </p>
        </div>
        {server ? (
          <Switch
            checked={requested}
            disabled={working}
            onCheckedChange={onToggle}
            aria-label={t("status.toggle")}
          />
        ) : (
          <Button size="sm" onClick={onEditServer}>{t("server.setup")}</Button>
        )}
      </div>

      {warning ? (
        <p className="border-t border-border px-4 py-2.5 text-xs text-warn">{warning}</p>
      ) : null}
    </Card>
  );
}
