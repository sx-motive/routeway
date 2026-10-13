import * as SwitchPrimitive from "@radix-ui/react-switch";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "../../lib/utils";

export const Switch = forwardRef<
  ElementRef<typeof SwitchPrimitive.Root>,
  ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitive.Root
    ref={ref}
    className={cn(
      "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center bg-fg-muted/25 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-40 data-[state=checked]:bg-accent",
      className,
    )}
    {...props}
  >
    <SwitchPrimitive.Thumb className="pointer-events-none block h-3.5 w-3.5 translate-x-[3px] bg-fg-muted transition-transform data-[state=checked]:translate-x-[19px] data-[state=checked]:bg-accent-fg" />
  </SwitchPrimitive.Root>
));
Switch.displayName = SwitchPrimitive.Root.displayName;
