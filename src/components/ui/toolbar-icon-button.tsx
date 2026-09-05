import * as React from "react";
import { cn } from "@/lib/utils";
import { IconButton, type IconButtonProps } from "./icon-button";
import styles from "./toolbar-icon-button.module.scss";

export type ToolbarIconButtonProps = IconButtonProps & {
  interactionMode?: "standard" | "native";
};

function clearNativePressed(button: HTMLButtonElement) {
  button.removeAttribute("data-native-pressed");
}

export const ToolbarIconButton = React.forwardRef<HTMLButtonElement, ToolbarIconButtonProps>(({
  className,
  interactionMode = "standard",
  onBlur,
  onClick,
  onPointerCancel,
  onPointerDown,
  onPointerLeave,
  onPointerUp,
  ...props
}, ref) => {
  const isNative = interactionMode === "native";
  const clearPressed = (event: React.SyntheticEvent<HTMLButtonElement>) => {
    if (isNative) clearNativePressed(event.currentTarget);
  };
  const handleBlur = (event: React.FocusEvent<HTMLButtonElement>) => {
    clearPressed(event);
    onBlur?.(event);
  };
  const handleClick = (event: React.MouseEvent<HTMLButtonElement>) => {
    clearPressed(event);
    // 鼠标点击不保留非聚焦窗口中的旧焦点；键盘触发的 click.detail 为 0，保留焦点环。
    if (isNative && event.detail > 0) event.currentTarget.blur();
    onClick?.(event);
  };
  const handlePointerCancel = (event: React.PointerEvent<HTMLButtonElement>) => {
    clearPressed(event);
    onPointerCancel?.(event);
  };
  const handlePointerDown = (event: React.PointerEvent<HTMLButtonElement>) => {
    if (isNative && event.button === 0 && !event.currentTarget.disabled) {
      event.currentTarget.setAttribute("data-native-pressed", "");
    }
    onPointerDown?.(event);
  };
  const handlePointerLeave = (event: React.PointerEvent<HTMLButtonElement>) => {
    clearPressed(event);
    onPointerLeave?.(event);
  };
  const handlePointerUp = (event: React.PointerEvent<HTMLButtonElement>) => {
    clearPressed(event);
    onPointerUp?.(event);
  };

  return (
    <IconButton
      ref={ref}
      className={cn(styles.button, className)}
      data-interaction-mode={isNative ? "native" : undefined}
      onBlur={handleBlur}
      onClick={handleClick}
      onPointerCancel={handlePointerCancel}
      onPointerDown={handlePointerDown}
      onPointerLeave={handlePointerLeave}
      onPointerUp={handlePointerUp}
      {...props}
    />
  );
});
ToolbarIconButton.displayName = "ToolbarIconButton";
