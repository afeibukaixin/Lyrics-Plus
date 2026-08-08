import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import styles from "../settings.module.scss";

const colorChoices = [
  "#f8fafc", "#e2e8f0", "#cbd5e1", "#aab7c8", "#8090a4",
  "#7dd3fc", "#a5b4fc", "#c4b5fd", "#99f6e4", "#fde68a", "#fda4af",
  "#172033", "#25324a", "#3d4b63", "#64748b",
];

export function SettingsHeading({ title, description, onReset, resetting = false, confirming = false }: { title: string; description: string; onReset?: () => void; resetting?: boolean; confirming?: boolean }) {
  const { t } = useTranslation();
  return <div className={styles.settingsHeading}><div><h2>{title}</h2><p>{description}</p></div>{onReset && <button data-confirming={confirming} disabled={resetting} onClick={onReset}>{resetting ? t("common.actions.resetting") : confirming ? t("common.actions.confirmAgain") : t("common.actions.resetDefault")}</button>}</div>;
}

export function SettingsCard({ title, trailing, children }: { title: string; trailing?: React.ReactNode; children: React.ReactNode }) {
  return <section className={styles.card}><header><h3>{title}</h3>{trailing}</header>{children}</section>;
}

export function ToggleRow({ label, description, value, onChange }: { label: string; description?: string; value: boolean; onChange: (value: boolean) => void | Promise<unknown> }) {
  return <div className={styles.settingRow}><div><strong>{label}</strong>{description && <small>{description}</small>}</div><button aria-label={label} aria-pressed={value} className={styles.switch} data-on={value} onClick={() => void onChange(!value)}><span /></button></div>;
}

export function RangeRow({ label, value, min, max, step = 1, suffix, displayValue, disabled = false, onChange }: { label: string; value: number; min: number; max: number; step?: number; suffix: string; displayValue?: number; disabled?: boolean; onChange: (value: number) => void }) {
  return <div className={styles.settingRow}><strong>{label}</strong><div className={styles.rangeControl}><input aria-label={label} disabled={disabled} type="range" min={min} max={max} step={step} value={value} onChange={(event) => onChange(Number(event.target.value))} /><b>{displayValue ?? value}{suffix}</b></div></div>;
}

export function ColorRow({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(value);
  const [invalid, setInvalid] = useState(false);

  useEffect(() => {
    setDraft(value);
    setInvalid(false);
  }, [value]);

  const applyDraft = () => {
    const next = draft.trim();
    if (!next || !CSS.supports("color", next)) {
      setInvalid(true);
      return;
    }
    onChange(next);
    setOpen(false);
  };

  return (
    <div className={`${styles.settingRow} ${styles.colorSettingRow}`}>
      <strong>{label}</strong>
      <button type="button" className={styles.colorTrigger} aria-expanded={open} aria-label={t("settings.common.colorSelect", { label })} onClick={() => setOpen((current) => !current)}>
        <span style={{ background: value }} /><code>{value}</code>
      </button>
      {open && (
        <div className={styles.colorPanel}>
          <div className={styles.colorPalette} aria-label={t("settings.common.presetColors", { label })}>
            {colorChoices.map((color) => <button type="button" aria-label={color} aria-pressed={color.toLowerCase() === value.toLowerCase()} key={color} onClick={() => { onChange(color); setOpen(false); }} style={{ background: color }} />)}
          </div>
          <form className={styles.colorValueForm} onSubmit={(event) => { event.preventDefault(); applyDraft(); }}>
            <input aria-invalid={invalid} aria-label={t("settings.common.colorValue", { label })} placeholder={t("settings.common.colorPlaceholder")} spellCheck={false} value={draft} onChange={(event) => { setDraft(event.target.value); setInvalid(false); }} />
            <button type="submit">{t("common.actions.apply")}</button>
          </form>
        </div>
      )}
    </div>
  );
}

export function SelectRow({ label, description, disabled = false, value, options, onChange }: { label: string; description?: string; disabled?: boolean; value: string; options: Array<[string, string]>; onChange: (value: string) => void }) {
  return <div className={styles.settingRow}><div><strong>{label}</strong>{description && <small>{description}</small>}</div><select aria-label={label} disabled={disabled} value={value} onChange={(event) => onChange(event.target.value)}>{options.map(([optionValue, text]) => <option value={optionValue} key={optionValue}>{text}</option>)}</select></div>;
}
