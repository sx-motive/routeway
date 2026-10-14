import { ListEditor } from "./ListEditor";
import { normalizeDomain } from "./DomainList";
import { useI18n } from "../i18n";

export function DirectList({
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
      title={t("direct.title")}
      placeholder="intranet.company.com"
      items={domains}
      onChange={onChange}
      onEditFile={onEditFile}
      normalize={normalizeDomain}
      rejectedLabel={t("domains.rejected")}
      emptyHint={
        <>
          {t("direct.empty1")}
          <br />
          {t("direct.empty2")}
        </>
      }
      footer={
        <>
          {rich("direct.footer", {
            a: <span className="font-mono">company.com</span>,
          })}
        </>
      }
    />
  );
}
