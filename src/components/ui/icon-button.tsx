import * as React from "react";
import { Button, type ButtonProps } from "./button";
import { Tooltip, TooltipContent, TooltipTrigger } from "./tooltip";

export type IconButtonProps = Omit<ButtonProps, "aria-label"> & {
  label: string;
  tooltip?: React.ReactNode;
};

export const IconButton = React.forwardRef<HTMLButtonElement, IconButtonProps>(({
  label,
  tooltip = label,
  size = "icon",
  type = "button",
  ...props
}, ref) => (
  <Tooltip>
    <TooltipTrigger render={<Button ref={ref} type={type} size={size} aria-label={label} {...props} />} />
    <TooltipContent>{tooltip}</TooltipContent>
  </Tooltip>
));
IconButton.displayName = "IconButton";
