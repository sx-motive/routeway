import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

/** A pill-shaped group of mutually exclusive buttons (tabs, language, …). */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  className,
  role,
}: {
  value: T;
  options: readonly { id: T; label: ReactNode }[];
  onChange: (value: T) => void;
  className?: string;
  role?: "tablist";
}) {
  return (
    <div role={role} className={cn("flex h-8 items-stretch rounded-full bg-bg-elevated p-0.5", className)}>
      {options.map(({ id, label }) => {
        const active = value === id;
        return (
          <button
            key={id}
            type="button"
            role={role ? "tab" : undefined}
            aria-selected={role ? active : undefined}
            aria-pressed={role ? undefined : active}
            onClick={() => onChange(id)}
            className={cn(
              "rounded-full px-3 text-xs transition-colors outline-none focus-visible:ring-2 focus-visible:ring-fg/25",
              active
                ? "bg-selected font-medium text-fg shadow-sm shadow-black/10"
                : "text-fg-muted hover:text-fg",
            )}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}
