import { Tooltip as TooltipPrimitive } from "@base-ui/react/tooltip";
import { cn } from "@/lib/utils";

type TooltipProviderProps = Omit<TooltipPrimitive.Provider.Props, "delay"> & { delay?: number; delayDuration?: number };

function TooltipProvider({ delay, delayDuration, ...props }: TooltipProviderProps) {
  return <TooltipPrimitive.Provider delay={delay ?? delayDuration ?? 400} {...props} />;
}

const Tooltip = TooltipPrimitive.Root;
const TooltipTrigger = TooltipPrimitive.Trigger;

type TooltipContentProps = TooltipPrimitive.Popup.Props & Pick<TooltipPrimitive.Positioner.Props, "align" | "alignOffset" | "side" | "sideOffset">;

function TooltipContent({ className, align = "center", alignOffset = 0, side = "top", sideOffset = 6, ...props }: TooltipContentProps) {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Positioner align={align} alignOffset={alignOffset} side={side} sideOffset={sideOffset} className="z-50">
        <TooltipPrimitive.Popup data-slot="tooltip-content" className={cn("max-w-xs origin-[var(--transform-origin)] rounded-md bg-popover px-2 py-1 text-xs leading-snug text-popover-foreground shadow-md ring-1 ring-foreground/10 transition-[opacity,transform] duration-100 data-[ending-style]:scale-95 data-[ending-style]:opacity-0 data-[starting-style]:scale-95 data-[starting-style]:opacity-0", className)} {...props} />
      </TooltipPrimitive.Positioner>
    </TooltipPrimitive.Portal>
  );
}

export { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger };
