import { ListEditor } from "./ListEditor";

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
}: {
  processes: string[];
  onChange: (processes: string[]) => void;
}) {
  return (
    <ListEditor
      title="Приложения через сервер"
      placeholder="codex"
      items={processes}
      onChange={onChange}
      normalize={normalizeProcess}
      rejectedLabel="Не похоже на имя процесса"
      emptyHint={
        <>
          Пока ни одно приложение не привязано к серверу.
          <br />
          Добавь имя процесса — например <span className="font-mono">codex</span>.
        </>
      }
      footer={
        <>
          Имя процесса (<span className="font-mono">codex</span>) или полный путь
          (<span className="font-mono">/usr/bin/curl</span>). Работает для приложений, чей трафик
          доходит до Routeway — то есть при включённом системном прокси.
        </>
      }
    />
  );
}
