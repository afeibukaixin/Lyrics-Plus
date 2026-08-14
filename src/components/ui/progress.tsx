import * as React from "react";
import { cn } from "@/lib/utils";
export function Progress({ value = 0, className, ...props }: React.HTMLAttributes<HTMLDivElement> & { value?: number }) { return <div role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={value} className={cn("h-1.5 w-full overflow-hidden rounded-full bg-secondary", className)} {...props}><div className="h-full bg-primary transition-transform" style={{ transform: `translateX(-${100 - Math.max(0, Math.min(100, value))}%)` }} /></div>; }
