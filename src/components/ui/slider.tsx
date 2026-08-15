import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cn } from "@/lib/utils";

function Slider({ className, "aria-label": ariaLabel, ...props }: SliderPrimitive.Root.Props<number>) {
  return (
    <SliderPrimitive.Root data-slot="slider" className={cn("relative flex w-full touch-none select-none items-center data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50", className)} {...props}>
      <SliderPrimitive.Control className="flex w-full items-center py-2">
        <SliderPrimitive.Track className="relative h-1 w-full grow rounded-full bg-muted">
          <SliderPrimitive.Indicator className="absolute h-full rounded-full bg-primary" />
          <SliderPrimitive.Thumb aria-label={ariaLabel} className="block size-3 rounded-full border border-ring bg-background ring-ring/50 outline-none transition-shadow hover:ring-3 has-[input:focus-visible]:ring-3" />
        </SliderPrimitive.Track>
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}

export { Slider };
