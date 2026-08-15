import { Switch as SwitchPrimitive } from "@base-ui/react/switch";
import { cn } from "@/lib/utils";

function Switch({ className, ...props }: SwitchPrimitive.Root.Props) {
  return (
    <SwitchPrimitive.Root data-slot="switch" className={cn("peer group/switch relative inline-flex h-[18.4px] w-8 shrink-0 cursor-pointer items-center rounded-full border border-transparent bg-input outline-none transition-all after:absolute after:-inset-x-3 after:-inset-y-2 data-[checked]:bg-primary focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50", className)} {...props}>
      <SwitchPrimitive.Thumb data-slot="switch-thumb" className="pointer-events-none block size-4 translate-x-0 rounded-full bg-background ring-0 transition-transform data-[checked]:translate-x-[calc(100%-2px)]" />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
