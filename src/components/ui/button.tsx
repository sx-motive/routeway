import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/utils";

const variants = {
  default: "bg-accent text-accent-fg hover:bg-accent/90",
  ghost: "text-fg-muted hover:bg-bg-elevated hover:text-fg",
  outline: "border border-border bg-transparent text-fg hover:bg-bg-elevated",
  danger: "border border-border bg-transparent text-danger hover:bg-bg-elevated",
};
const sizes = {
  default: "h-9 px-4",
  sm: "h-8 px-3 text-xs",
  icon: "h-8 w-8",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: keyof typeof variants;
  size?: keyof typeof sizes;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = "default", size = "default", ...props }, ref) => (
    <button
      ref={ref}
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-md text-sm font-medium select-none transition-colors disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  ),
);
Button.displayName = "Button";
