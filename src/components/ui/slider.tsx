import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cn } from "@/lib/utils";

type SliderValue = number | readonly number[];

type SliderProps<Value extends SliderValue> = SliderPrimitive.Root.Props<Value> & {
  "aria-label"?: string;
  thumbLabels?: string[];
};

function Slider<Value extends SliderValue>({
  className,
  "aria-label": ariaLabel,
  thumbLabels,
  value,
  defaultValue,
  min = 0,
  max = 100,
  ...props
}: SliderProps<Value>) {
  const thumbCount = Array.isArray(value)
    ? value.length
    : Array.isArray(defaultValue)
      ? defaultValue.length
      : 1;

  return (
    <SliderPrimitive.Root
      data-slot="slider"
      className={cn("relative flex w-full touch-none select-none items-center data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50", className)}
      defaultValue={defaultValue}
      max={max}
      min={min}
      value={value}
      {...props}
    >
      <SliderPrimitive.Control className="relative flex w-full items-center py-2">
        <SliderPrimitive.Track className="relative h-1 w-full grow rounded-full bg-muted">
          <SliderPrimitive.Indicator className="absolute h-full rounded-full bg-primary" />
        </SliderPrimitive.Track>
        {Array.from({ length: thumbCount }, (_, index) => (
          <SliderPrimitive.Thumb
            aria-label={thumbLabels?.[index] ?? ariaLabel}
            index={thumbCount > 1 ? index : undefined}
            key={index}
            className="block size-3 rounded-full border border-ring bg-background ring-ring/50 outline-none transition-shadow hover:ring-3 has-[input:focus-visible]:ring-3"
          />
        ))}
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}

export { Slider };
