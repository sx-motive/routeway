import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

export function Card({
  title,
  aside,
  className,
  children,
}: {
  title?: ReactNode;
  aside?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section
      className={cn(
        "overflow-hidden rounded-2xl border border-border bg-surface shadow-[var(--card-shadow)]",
        className,
      )}
    >
      {title ? (
        <header className="flex h-11 shrink-0 items-center justify-between gap-3 border-b border-border px-4">
          <h2 className="truncate text-xs font-medium text-fg-muted">
            {title}
          </h2>
          {aside}
        </header>
      ) : null}
      {children}
    </section>
  );
}

export function Rows({ children }: { children: ReactNode }) {
  return <div className="divide-y divide-border">{children}</div>;
}

export function Row({
  label,
  hint,
  control,
  interactive = true,
}: {
  label: ReactNode;
  hint?: ReactNode;
  control: ReactNode;
  interactive?: boolean;
}) {
  const Root = interactive ? "label" : "div";
  return (
    <Root className="flex min-h-14 items-center justify-between gap-6 px-4 py-3">
      <span className="min-w-0">
        <span className="block text-sm text-fg">{label}</span>
        {hint ? (
          <span className="mt-0.5 block text-xs leading-relaxed text-fg-muted">{hint}</span>
        ) : null}
      </span>
      <span className="flex shrink-0 items-center">{control}</span>
    </Root>
  );
}
