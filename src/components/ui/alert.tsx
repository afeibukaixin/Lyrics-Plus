import * as React from "react";
import { cn } from "@/lib/utils";
export const Alert = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(({ className, ...props }, ref) => <div ref={ref} role="alert" className={cn("relative rounded-lg border border-border bg-card p-4 text-sm", className)} {...props} />);
export const AlertTitle = React.forwardRef<HTMLHeadingElement, React.HTMLAttributes<HTMLHeadingElement>>(({ className, ...props }, ref) => <h5 ref={ref} className={cn("mb-1 font-medium", className)} {...props} />);
export const AlertDescription = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(({ className, ...props }, ref) => <div ref={ref} className={cn("text-muted-foreground", className)} {...props} />);
