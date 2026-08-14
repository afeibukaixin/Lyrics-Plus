import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";
const badgeVariants = cva("inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium", { variants: { variant: { default: "border-transparent bg-primary text-primary-foreground", secondary: "border-transparent bg-secondary text-secondary-foreground", outline: "text-foreground", destructive: "border-transparent bg-destructive text-white" } }, defaultVariants: { variant: "default" } });
export function Badge({ className, variant, ...props }: React.HTMLAttributes<HTMLSpanElement> & VariantProps<typeof badgeVariants>) { return <span className={cn(badgeVariants({ variant }), className)} {...props} />; }
