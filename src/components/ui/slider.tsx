import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cn } from "@/lib/utils";

function Slider({ className, ...props }: SliderPrimitive.Root.Props<readonly number[]>) {
  return (
    <SliderPrimitive.Root data-slot="slider" className={cn("relative flex w-full touch-none select-none items-center data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50", className)} {...props}>
      <SliderPrimitive.Control className="flex w-full items-center py-2">
        <SliderPrimitive.Track className="relative h-1 w-full grow overflow-hidden rounded-full bg-muted">
          <SliderPrimitive.Indicator className="absolute h-full bg-primary" />
        </SliderPrimitive.Track>
        <SliderPrimitive.Thumb className="block size-3 rounded-full border border-ring bg-background ring-ring/50 outline-none transition-shadow hover:ring-3 focus-visible:ring-3" />
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}

export { Slider };
