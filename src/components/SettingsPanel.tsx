import { useState, type MouseEvent } from "react";
import { Loader2, RefreshCw, ShieldCheck } from "lucide-react";
import { ServerCard } from "./ServerCard";
import { DomainList } from "./DomainList";
import { DirectList } from "./DirectList";
import { ProcessList } from "./ProcessList";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";
import { Card, Row, Rows } from "./ui/card";
import { NumericInput } from "./ui/numeric-input";
import { LANGS, useI18n, type Lang } from "../i18n";
import type { MaintenanceAction, Settings, Status } from "../lib/ipc";
import type { Server } from "../lib/vless";
import appLogo from "../assets/brand/128x128.png";
import { license, version } from "../../package.json";
import { openExternal } from "../lib/ipc";

interface SettingsPanelProps {
  status: Status;
  working: boolean;
  persist: (settings: Settings) => Promise<void>;
  switchTun: (settings: Settings) => Promise<void>;
  changeLanguage: (lang: Lang) => Promise<void>;
  onEditFile: () => void;
  ips: { direct?: string; server?: string };
  probing: boolean;
  probe: () => Promise<void>;
  maintainCore: (action: MaintenanceAction) => Promise<void>;
}

export default function SettingsPanel({
  status, working, persist, switchTun, changeLanguage, onEditFile, ips, probing, probe, maintainCore,
}: SettingsPanelProps) {
  const { settings, running } = status;
  const { t, lang } = useI18n();
  const [linkError, setLinkError] = useState<string | null>(null);

  function openProjectLink(event: MouseEvent<HTMLAnchorElement>) {
    event.preventDefault();
    setLinkError(null);
    void openExternal(event.currentTarget.href).catch((err) => setLinkError(String(err)));
  }
  return (
    <>
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
          <Row
            label={t("settings.autoConnect")}
            control={
              <Switch
                checked={settings.auto_connect}
                onCheckedChange={(on) => void persist({ ...settings, auto_connect: on })}
                aria-label={t("settings.autoConnect")}
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
        onEditFile={onEditFile}
      />

      <DomainList
        domains={settings.domains}
        onChange={(domains) => void persist({ ...settings, domains })}
        onEditFile={onEditFile}
      />

      <DirectList
        domains={settings.direct}
        onChange={(direct) => void persist({ ...settings, direct })}
        onEditFile={onEditFile}
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
      <DiagnosticsPanel working={working} canRestart={Boolean(settings.server)} maintainCore={maintainCore} />
      <Card title={t("about.title")}>
        <div className="flex flex-col items-center px-6 py-6 text-center">
          <img src={appLogo} alt="" width={64} height={64} draggable={false} className="brand-logo size-16" />
          <h3 className="mt-3 text-sm font-medium text-fg">Routeway</h3>
          <p className="mt-1 text-xs text-fg-muted">{t("about.version", { version })}</p>
          <p className="mt-3 text-xs leading-relaxed text-fg-muted">
            © {new Date().getFullYear()} Routeway · Open source · {t("about.license")} {" "}
            <a href="https://www.gnu.org/licenses/gpl-3.0.html" target="_blank" rel="noopener noreferrer"
              onClick={openProjectLink}
              className="underline underline-offset-4 hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
              {license}
            </a>
            {". "}{t("about.noWarranty")}
          </p>
          <nav aria-label={t("about.links")} className="mt-4 flex flex-wrap justify-center gap-x-5 gap-y-2 text-xs">
            {([
              ["GitHub", "https://github.com/sx-motive/routeway"],
              [t("about.releases"), "https://github.com/sx-motive/routeway/releases"],
              [t("about.issues"), "https://github.com/sx-motive/routeway/issues"],
            ] as const).map(([label, href]) => (
              <a key={href} href={href} target="_blank" rel="noopener noreferrer"
                onClick={openProjectLink}
                className="text-fg-muted underline-offset-4 transition-colors hover:text-fg hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                {label}
              </a>
            ))}
          </nav>
          {linkError ? <p role="alert" className="mt-3 break-words text-xs text-danger">{t("about.linkError", { error: linkError })}</p> : null}
        </div>
      </Card>
    </>
  );
}
