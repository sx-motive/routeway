import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/utils";

const variants = {
  default: "bg-accent text-accent-fg shadow-sm shadow-accent/20 hover:bg-accent/90",
  ghost: "text-fg-muted hover:bg-bg-elevated hover:text-fg",
  outline: "bg-bg-elevated text-fg hover:bg-fg/10",
  danger: "bg-danger/10 text-danger hover:bg-danger/15",
};
const sizes = {
  default: "h-9 px-4",
  sm: "h-8 px-3.5 text-xs",
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
        "inline-flex shrink-0 whitespace-nowrap [&_svg]:shrink-0 items-center justify-center gap-2 rounded-full text-sm font-medium select-none transition-colors disabled:pointer-events-none disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  ),
);
Button.displayName = "Button";
