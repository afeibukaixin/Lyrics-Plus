import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Music2, X } from "lucide-react";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Input } from "../../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../components/ui/select";
import { Slider } from "../../components/ui/slider";
import { Switch } from "../../components/ui/switch";
import type { RegisteredApplication } from "../../shared/types";
import styles from "../settings.module.scss";

const colorChoices = [
  "#f8fafc", "#e2e8f0", "#cbd5e1", "#aab7c8", "#8090a4",
  "#7dd3fc", "#a5b4fc", "#c4b5fd", "#99f6e4", "#fde68a", "#fda4af",
  "#172033", "#25324a", "#3d4b63", "#64748b",
];

export function SettingsHeading({ title, description, onReset, resetting = false }: { title: string; description: string; onReset?: () => void; resetting?: boolean; confirming?: boolean }) {
  const { t } = useTranslation();
  return <div className={styles.settingsHeading}><div><h2>{title}</h2><p>{description}</p></div>{onReset && <Button variant="outline" size="sm" disabled={resetting} onClick={onReset}>{resetting ? t("common.actions.resetting") : t("common.actions.resetDefault")}</Button>}</div>;
}

export function SettingsCard({ title, trailing, children }: { title: string; trailing?: React.ReactNode; children: React.ReactNode }) {
  return <Card className={styles.card}><header><h3>{title}</h3>{trailing}</header>{children}</Card>;
}

export function ApplicationList({ applications, icons, busy, emptyLabel, removeLabel, onRemove }: {
  applications: RegisteredApplication[];
  icons: Record<string, string>;
  busy: boolean;
  emptyLabel: string;
  removeLabel: string;
  onRemove: (bundleId: string) => void;
}) {
  if (applications.length === 0) return <p className={styles.applicationEmpty}>{emptyLabel}</p>;
  return <div className={styles.applicationList}>{applications.map((application) => (
    <div className={styles.applicationItem} key={application.bundleId} title={application.bundleId}>
      {icons[application.bundleId]
        ? <img alt="" src={icons[application.bundleId]} />
        : <span className={styles.applicationIconFallback}><Music2 /></span>}
      <div className={styles.applicationDetails}>
        <strong>{application.name}</strong>
        <small>{application.bundleId}</small>
      </div>
      <button aria-label={`${removeLabel} ${application.name}`} className={styles.applicationRemove} disabled={busy} title={removeLabel} onClick={() => onRemove(application.bundleId)}><X /></button>
    </div>
  ))}</div>;
}

export function ToggleRow({ label, description, value, disabled = false, onChange }: { label: string; description?: string; value: boolean; disabled?: boolean; onChange: (value: boolean) => void | Promise<unknown> }) {
  return <div className={styles.settingRow}><div><strong>{label}</strong>{description && <small>{description}</small>}</div><Switch aria-label={label} checked={value} disabled={disabled} onCheckedChange={(checked) => void onChange(checked)} /></div>;
}

export function RangeRow({ label, value, min, max, step = 1, suffix, displayValue, disabled = false, onChange }: { label: string; value: number; min: number; max: number; step?: number; suffix: string; displayValue?: number; disabled?: boolean; onChange: (value: number) => void }) {
  return <div className={styles.settingRow}><strong>{label}</strong><div className={styles.rangeControl}><Slider aria-label={label} disabled={disabled} min={min} max={max} step={step} value={[value]} onValueChange={([next]) => onChange(next)} /><b>{displayValue ?? value}{suffix}</b></div></div>;
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
            <Input aria-invalid={invalid} aria-label={t("settings.common.colorValue", { label })} placeholder={t("settings.common.colorPlaceholder")} spellCheck={false} value={draft} onChange={(event) => { setDraft(event.target.value); setInvalid(false); }} />
            <Button type="submit" size="sm">{t("common.actions.apply")}</Button>
          </form>
        </div>
      )}
    </div>
  );
}

export function SelectRow({ label, description, disabled = false, value, options, onChange }: { label: string; description?: string; disabled?: boolean; value: string; options: Array<[string, string]>; onChange: (value: string) => void }) {
  return <div className={styles.settingRow}><div><strong>{label}</strong>{description && <small>{description}</small>}</div><Select disabled={disabled} value={value} onValueChange={onChange}><SelectTrigger aria-label={label} className="w-[180px]"><SelectValue /></SelectTrigger><SelectContent>{options.map(([optionValue, text]) => <SelectItem value={optionValue} key={optionValue}>{text}</SelectItem>)}</SelectContent></Select></div>;
}
