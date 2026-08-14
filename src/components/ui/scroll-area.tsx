import * as React from "react";
import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area";
import { cn } from "@/lib/utils";
export const ScrollArea = React.forwardRef<React.ElementRef<typeof ScrollAreaPrimitive.Root>, React.ComponentPropsWithoutRef<typeof ScrollAreaPrimitive.Root>>(({ className, children, ...props }, ref) => <ScrollAreaPrimitive.Root ref={ref} className={cn("relative overflow-hidden", className)} {...props}><ScrollAreaPrimitive.Viewport className="size-full rounded-[inherit]">{children}</ScrollAreaPrimitive.Viewport><ScrollAreaPrimitive.Scrollbar orientation="vertical" className="flex w-2.5 touch-none select-none p-px"><ScrollAreaPrimitive.Thumb className="relative flex-1 rounded-full bg-border" /></ScrollAreaPrimitive.Scrollbar><ScrollAreaPrimitive.Corner /></ScrollAreaPrimitive.Root>);
ScrollArea.displayName = ScrollAreaPrimitive.Root.displayName;
