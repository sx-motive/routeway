import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "../../lib/utils";

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(
        "h-9 w-full border border-border bg-bg px-3 text-sm text-fg placeholder:text-fg-muted/60 transition-colors focus-visible:outline-none focus-visible:border-accent focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-50",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs text-fg-muted">
        {label}
        {hint ? <span className="text-fg-muted/60"> · {hint}</span> : null}
      </span>
      {children}
    </label>
  );
}
