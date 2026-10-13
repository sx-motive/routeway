import { ListEditor } from "./ListEditor";
import { useI18n } from "../i18n";

/** Mirrors the Rust-side normalizer so the UI shows what will actually be stored. */
export function normalizeDomain(raw: string): string | null {
  let value = raw.trim().toLowerCase();
  if (!value || value.startsWith("#")) return null;
  const schemeAt = value.indexOf("://");
  if (schemeAt >= 0) value = value.slice(schemeAt + 3);
  value = value.split("/")[0].split("?")[0];
  const at = value.lastIndexOf("@");
  if (at >= 0) value = value.slice(at + 1);
  const colon = value.indexOf(":");
  if (colon >= 0) value = value.slice(0, colon);
  value = value.replace(/^\.+|\.+$/g, "");
  if (!value || !value.includes(".")) return null;
  return value;
}

export function DomainList({
  domains,
  onChange,
  onEditFile,
}: {
  domains: string[];
  onChange: (domains: string[]) => void;
  onEditFile: () => void;
}) {
  const { t, rich } = useI18n();
  return (
    <ListEditor
      title={t("domains.title")}
      placeholder="youtube.com"
      items={domains}
      onChange={onChange}
      onEditFile={onEditFile}
      normalize={normalizeDomain}
      rejectedLabel={t("domains.rejected")}
      emptyHint={
        <>
          {t("domains.empty1")}
          <br />
          {t("domains.empty2")}
        </>
      }
      footer={
        <>
          {rich("domains.footer", {
            a: <span className="font-mono">youtube.com</span>,
            b: <span className="font-mono">www.youtube.com</span>,
          })}
        </>
      }
    />
  );
}
