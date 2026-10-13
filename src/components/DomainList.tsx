import { ListEditor } from "./ListEditor";

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
}: {
  domains: string[];
  onChange: (domains: string[]) => void;
}) {
  return (
    <ListEditor
      title="Домены через сервер"
      placeholder="youtube.com"
      items={domains}
      onChange={onChange}
      normalize={normalizeDomain}
      rejectedLabel="Не похоже на домен"
      emptyHint={
        <>
          Пока ни один домен не идёт через сервер.
          <br />
          Добавь домен — и только он пойдёт в туннель.
        </>
      }
      footer={
        <>
          Домен включает все поддомены: <span className="font-mono">youtube.com</span> покрывает и{" "}
          <span className="font-mono">www.youtube.com</span>.
        </>
      }
    />
  );
}
