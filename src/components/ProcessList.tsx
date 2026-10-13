import { ListEditor } from "./ListEditor";
import { useI18n } from "../i18n";

/**
 * Process entries are either a bare executable name ("codex") or an absolute
 * path ("/usr/bin/curl"). Unlike domains, case is preserved — process names are
 * case-sensitive on macOS.
 */
export function normalizeProcess(raw: string): string | null {
  const value = raw.trim().replace(/^["']|["']$/g, "");
  if (!value || value.startsWith("#")) return null;
  // A lone dot or slash is a typo, not a target.
  if (/^[./\\]+$/.test(value)) return null;
  return value;
}

export function ProcessList({
  processes,
  onChange,
  onEditFile,
}: {
  processes: string[];
  onChange: (processes: string[]) => void;
  onEditFile: () => void;
}) {
  const { t, rich } = useI18n();
  return (
    <ListEditor
      title={t("apps.title")}
      placeholder="codex"
      items={processes}
      onChange={onChange}
      onEditFile={onEditFile}
      normalize={normalizeProcess}
      rejectedLabel={t("apps.rejected")}
      emptyHint={
        <>
          {t("apps.empty1")}
          <br />
          {rich("apps.empty2", { example: <span className="font-mono">codex</span> })}
        </>
      }
      footer={
        <>
          {rich("apps.footer", {
            name: <span className="font-mono">codex</span>,
            path: <span className="font-mono">/usr/bin/curl</span>,
          })}
        </>
      }
    />
  );
}
