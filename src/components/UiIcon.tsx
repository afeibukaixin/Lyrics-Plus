import styles from "./UiIcon.module.scss";

const iconClassNames = {
  bracketsCurly: "i-ph-brackets-curly",
  bug: "i-ph-bug",
  checkerboard: "i-ph-checkerboard",
  close: "i-ph-x",
  columns: "i-ph-columns",
  drag: "i-ph-dots-six-vertical",
  eyeSlash: "i-ph-eye-slash",
  fileText: "i-ph-file-text",
  gear: "i-ph-gear",
  info: "i-ph-info",
  lock: "i-ph-lock",
  lockOpen: "i-ph-lock-open",
  minus: "i-ph-minus",
  monitor: "i-ph-monitor",
  musicNote: "i-ph-music-note",
  musicNotes: "i-ph-music-notes",
  plus: "i-ph-plus",
  rows: "i-ph-rows",
  search: "i-ph-magnifying-glass",
  selectionBackground: "i-ph-selection-background",
  spinner: "i-ph-circle-notch",
  textAlignLeft: "i-ph-text-align-left",
  textColumns: "i-ph-text-columns",
} as const;

export type UiIconName = keyof typeof iconClassNames;

type UiIconProps = {
  className?: string;
  name: UiIconName;
  spin?: boolean;
};

export function UiIcon({ className, name, spin = false }: UiIconProps) {
  return (
    <span
      aria-hidden="true"
      className={[styles.icon, iconClassNames[name], spin && styles.spin, className].filter(Boolean).join(" ")}
    />
  );
}
