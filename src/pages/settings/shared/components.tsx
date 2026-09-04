import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldContent, FieldDescription, FieldTitle } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import styles from "../settings.module.scss";

export type SettingsPageSection = {
  id: string;
  label: string;
};

const colorChoices = [
  "#f8fafc", "#e2e8f0", "#cbd5e1", "#aab7c8", "#8090a4",
  "#7dd3fc", "#a5b4fc", "#c4b5fd", "#99f6e4", "#fde68a", "#fda4af",
  "#172033", "#25324a", "#3d4b63", "#64748b",
];

function normalizeHexColor(value: string) {
  const match = value.trim().match(/^#([\da-f]{3,4}|[\da-f]{6}|[\da-f]{8})$/i);
  if (!match) return null;
  const channels = match[1].length <= 4
    ? match[1].slice(0, 3).split("").map((channel) => channel.repeat(2)).join("")
    : match[1].slice(0, 6);
  return `#${channels.toLowerCase()}`;
}

function nativeColorValue(value: string) {
  const directHex = normalizeHexColor(value);
  if (directHex) return directHex;
  if (typeof document === "undefined" || !CSS.supports("color", value)) return "#000000";

  const context = document.createElement("canvas").getContext("2d");
  if (!context) return "#000000";
  context.fillStyle = "#000000";
  context.fillStyle = value;
  const normalized = context.fillStyle;
  const canvasHex = normalizeHexColor(normalized);
  if (canvasHex) return canvasHex;

  const channels = normalized.startsWith("rgb") ? normalized.match(/[\d.]+/g)?.slice(0, 3) : null;
  if (!channels || channels.length !== 3) return "#000000";
  return `#${channels.map((channel) => Math.round(Number(channel)).toString(16).padStart(2, "0")).join("")}`;
}

export function PageHeader({ title, description, onReset, resetLabel, resetting = false }: { title: string; description?: string; onReset?: () => void; resetLabel?: string; resetting?: boolean; confirming?: boolean }) {
  const { t } = useTranslation();
  return (
    <header className={styles.settingsHeading}>
      <div>
        <h1 className="text-2xl font-semibold tracking-tight text-balance">{title}</h1>
        {description && <p className="text-sm text-muted-foreground">{description}</p>}
      </div>
      {onReset && <Button variant="outline" size="sm" disabled={resetting} onClick={onReset}>{resetting ? t("common.actions.resetting") : resetLabel ?? t("common.actions.resetDefault")}</Button>}
    </header>
  );
}

export function SettingsPage({ sections, children }: { sections: SettingsPageSection[]; children: React.ReactNode }) {
  const { t } = useTranslation();
  const page = useRef<HTMLDivElement>(null);
  const sectionIds = sections.map(({ id }) => id).join("\u0000");
  const showTableOfContents = sections.length >= 2;
  const [activeSection, setActiveSection] = useState(sections[0]?.id ?? "");

  useEffect(() => {
    if (!showTableOfContents) return;
    const pageElement = page.current;
    const scrollRoot = pageElement?.closest<HTMLElement>("[data-settings-scroll-root]");
    const sectionElements = sectionIds
      .split("\u0000")
      .map((id) => document.getElementById(id))
      .filter((element): element is HTMLElement => Boolean(element));
    if (!pageElement || !scrollRoot || sectionElements.length === 0) return;

    let frame = 0;
    const updateActiveSection = () => {
      frame = 0;
      const rootTop = scrollRoot.getBoundingClientRect().top;
      const activationLine = rootTop + 80;
      const atBottom = scrollRoot.scrollTop + scrollRoot.clientHeight >= scrollRoot.scrollHeight - 2;
      let nextSection = sectionElements[0].id;

      if (atBottom) {
        nextSection = sectionElements[sectionElements.length - 1]?.id ?? nextSection;
      } else {
        for (const element of sectionElements) {
          if (element.getBoundingClientRect().top > activationLine) break;
          nextSection = element.id;
        }
      }
      setActiveSection((current) => current === nextSection ? current : nextSection);
    };
    const scheduleUpdate = () => {
      if (frame) return;
      frame = window.requestAnimationFrame(updateActiveSection);
    };
    const resizeObserver = new ResizeObserver(scheduleUpdate);

    resizeObserver.observe(pageElement);
    scrollRoot.addEventListener("scroll", scheduleUpdate, { passive: true });
    window.addEventListener("resize", scheduleUpdate);
    updateActiveSection();

    return () => {
      if (frame) window.cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      scrollRoot.removeEventListener("scroll", scheduleUpdate);
      window.removeEventListener("resize", scheduleUpdate);
    };
  }, [sectionIds, showTableOfContents]);

  const jumpToSection = (id: string) => {
    const element = document.getElementById(id);
    if (!element) return;
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    setActiveSection(id);
    element.scrollIntoView({ behavior: reduceMotion ? "auto" : "smooth", block: "start" });
  };

  return (
    <div ref={page} className={cn(styles.settingsPage, showTableOfContents && styles.settingsPageWithToc)}>
      <div className={styles.settingsPageContent}>{children}</div>
      {showTableOfContents && (
        <aside className={styles.settingsToc}>
          <nav aria-label={t("settings.shell.onThisPage")}>
            <p className={styles.settingsTocTitle}>{t("settings.shell.onThisPage")}</p>
            <div className={styles.settingsTocLinks}>
              {sections.map(({ id, label }) => {
                const active = activeSection === id;
                return (
                  <Button
                    key={id}
                    type="button"
                    variant={active ? "secondary" : "ghost"}
                    size="sm"
                    className="w-full justify-start"
                    aria-current={active ? "location" : undefined}
                    onClick={() => jumpToSection(id)}
                  >
                    <span className="truncate">{label}</span>
                  </Button>
                );
              })}
            </div>
          </nav>
        </aside>
      )}
    </div>
  );
}

export function SettingsSection({ id, title, trailing, children }: { id: string; title?: string; trailing?: React.ReactNode; children: React.ReactNode }) {
  return (
    <section id={id} className={styles.settingsSection}>
      <Card className={cn(styles.card, "gap-0 py-0")}>
        {(title || trailing) && (
          <CardHeader className={cn(styles.cardHeader, "border-b pt-(--card-spacing)")}>
            {title && <CardTitle>{title}</CardTitle>}
            {trailing && <CardAction>{trailing}</CardAction>}
          </CardHeader>
        )}
        <CardContent className={styles.cardContent}>{children}</CardContent>
      </Card>
    </section>
  );
}

export function ToggleRow({ label, description, value, disabled = false, onChange }: { label: string; description?: string; value: boolean; disabled?: boolean; onChange: (value: boolean) => void | Promise<unknown> }) {
  return (
    <Field orientation="horizontal" className={styles.settingRow} data-disabled={disabled || undefined}>
      <FieldContent>
        <FieldTitle>{label}</FieldTitle>
        {description && <FieldDescription>{description}</FieldDescription>}
      </FieldContent>
      <Switch aria-label={label} checked={value} disabled={disabled} onCheckedChange={(checked) => void onChange(checked)} />
    </Field>
  );
}

type RangeRowProps = {
  label: string;
  description?: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  suffix: string;
  displayValue?: number;
  disabled?: boolean;
  onChange: (value: number) => void;
  onValuePreview?: (value: number) => void;
  onValueCommitted?: (value: number) => void | Promise<void>;
  onPreviewCanceled?: () => void;
};

export function RangeRow({
  label,
  description,
  value,
  min,
  max,
  step = 1,
  suffix,
  displayValue,
  disabled = false,
  onChange,
  onValuePreview,
  onValueCommitted,
  onPreviewCanceled,
}: RangeRowProps) {
  const commitMode = Boolean(onValueCommitted);
  const [draft, setDraft] = useState(value);
  const previewingRef = useRef(false);
  const cancelPreviewRef = useRef(onPreviewCanceled);

  useEffect(() => {
    cancelPreviewRef.current = onPreviewCanceled;
  }, [onPreviewCanceled]);

  useEffect(() => {
    if (!previewingRef.current) setDraft(value);
  }, [value]);

  useEffect(() => () => {
    if (previewingRef.current) cancelPreviewRef.current?.();
  }, []);

  const renderedValue = commitMode ? draft : value;
  const handleValueChange = (next: number) => {
    if (!commitMode) {
      onChange(next);
      return;
    }
    previewingRef.current = true;
    setDraft(next);
    onValuePreview?.(next);
  };
  const handleValueCommitted = (next: number) => {
    if (!commitMode || !onValueCommitted) return;
    previewingRef.current = false;
    setDraft(next);
    void Promise.resolve()
      .then(() => onValueCommitted(next))
      .catch(() => {
        setDraft(value);
        cancelPreviewRef.current?.();
      });
  };

  return (
    <Field orientation="horizontal" className={styles.settingRow} data-disabled={disabled || undefined}>
      <FieldContent>
        <FieldTitle>{label}</FieldTitle>
        {description && <FieldDescription>{description}</FieldDescription>}
      </FieldContent>
      <div className={styles.rangeControl}>
        <Slider aria-label={label} disabled={disabled} min={min} max={max} step={step} value={renderedValue} onValueChange={handleValueChange} onValueCommitted={handleValueCommitted} />
        <output className="text-xs text-muted-foreground tabular-nums">{displayValue ?? renderedValue}{suffix}</output>
      </div>
    </Field>
  );
}

type RangePairRowProps = {
  label: string;
  firstLabel: string;
  secondLabel: string;
  values: readonly [number, number];
  min: number;
  max: number;
  step?: number;
  suffix: string;
  disabled?: boolean;
  normalizeValues?: (values: [number, number]) => [number, number];
  onValuePreview?: (values: [number, number]) => void;
  onValueCommitted?: (values: [number, number]) => void | Promise<void>;
  onPreviewCanceled?: () => void;
};

export function RangePairRow({
  label,
  firstLabel,
  secondLabel,
  values,
  min,
  max,
  step = 1,
  suffix,
  disabled = false,
  normalizeValues = (next) => next,
  onValuePreview,
  onValueCommitted,
  onPreviewCanceled,
}: RangePairRowProps) {
  const [draft, setDraft] = useState<[number, number]>(() => [values[0], values[1]]);
  const previewingRef = useRef(false);
  const cancelPreviewRef = useRef(onPreviewCanceled);

  useEffect(() => {
    cancelPreviewRef.current = onPreviewCanceled;
  }, [onPreviewCanceled]);

  useEffect(() => {
    if (!previewingRef.current) setDraft([values[0], values[1]]);
  }, [values[0], values[1]]);

  useEffect(() => () => {
    if (previewingRef.current) cancelPreviewRef.current?.();
  }, []);

  const pairFromValue = (next: number | readonly number[]): [number, number] => {
    if (typeof next === "number") return [next, draft[1]];
    return [next[0] ?? draft[0], next[1] ?? draft[1]];
  };

  const handleValueChange = (next: number | readonly number[]) => {
    const nextValues = normalizeValues(pairFromValue(next));
    previewingRef.current = true;
    setDraft(nextValues);
    onValuePreview?.(nextValues);
  };

  const handleValueCommitted = (next: number | readonly number[]) => {
    if (!onValueCommitted) return;
    const nextValues = normalizeValues(pairFromValue(next));
    previewingRef.current = false;
    setDraft(nextValues);
    void Promise.resolve()
      .then(() => onValueCommitted(nextValues))
      .catch(() => {
        setDraft([values[0], values[1]]);
        cancelPreviewRef.current?.();
      });
  };

  return (
    <Field orientation="horizontal" className={styles.settingRow} data-disabled={disabled || undefined}>
      <FieldContent>
        <FieldTitle>{label}</FieldTitle>
      </FieldContent>
      <div className={styles.rangePairControl}>
        <Slider
          aria-label={label}
          disabled={disabled}
          max={max}
          min={min}
          step={step}
          thumbLabels={[firstLabel, secondLabel]}
          thumbCollisionBehavior="push"
          value={draft}
          onValueChange={handleValueChange}
          onValueCommitted={handleValueCommitted}
        />
        <output className={styles.rangePairOutput}>
          <span>{firstLabel} {draft[0]}{suffix}</span>
          <span>{secondLabel} {draft[1]}{suffix}</span>
        </output>
      </div>
    </Field>
  );
}

export function TextRow({ label, description, value, emptyValue, disabled = false, onChange }: { label: string; description?: string; value: string; emptyValue: string; disabled?: boolean; onChange: (value: string) => void }) {
  const [draft, setDraft] = useState(value);

  useEffect(() => setDraft(value), [value]);

  const commit = () => {
    const next = draft.trim() || emptyValue;
    setDraft(next);
    if (next !== value) onChange(next);
  };

  return (
    <Field className={styles.textSettingRow} data-disabled={disabled || undefined}>
      <FieldContent>
        <FieldTitle>{label}</FieldTitle>
        {description && <FieldDescription>{description}</FieldDescription>}
      </FieldContent>
      <Input aria-label={label} disabled={disabled} spellCheck={false} value={draft} onBlur={commit} onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} />
    </Field>
  );
}

export function ColorRow({ label, description, value, disabled = false, onChange }: { label: string; description?: string; value: string; disabled?: boolean; onChange: (value: string) => void }) {
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
    <Field orientation="horizontal" className={cn(styles.settingRow, styles.colorSettingRow)} data-disabled={disabled || undefined} data-invalid={invalid || undefined}>
      <FieldContent>
        <FieldTitle>{label}</FieldTitle>
        {description && <FieldDescription>{description}</FieldDescription>}
      </FieldContent>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger render={<Button type="button" variant="outline" className={styles.colorTrigger} disabled={disabled} />}>
          <span style={{ background: value }} /><code>{value}</code>
        </PopoverTrigger>
        <PopoverContent align="end" className={styles.colorPopover}>
          <div className={styles.colorPalette} aria-label={t("settings.common.presetColors", { label })}>
            {colorChoices.map((color) => <Button type="button" variant="ghost" size="icon-sm" aria-label={color} aria-pressed={color.toLowerCase() === value.toLowerCase()} disabled={disabled} key={color} onClick={() => { onChange(color); setOpen(false); }} style={{ background: color }} />)}
          </div>
          <form onSubmit={(event) => { event.preventDefault(); applyDraft(); }}>
            <InputGroup>
              <InputGroupAddon>
                <Input
                  type="color"
                  className={styles.systemColorPicker}
                  aria-label={t("settings.common.systemColorPicker", { label })}
                  disabled={disabled}
                  value={nativeColorValue(value)}
                  onChange={(event) => { onChange(event.target.value); setOpen(false); }}
                />
              </InputGroupAddon>
              <InputGroupInput aria-invalid={invalid} aria-label={t("settings.common.colorValue", { label })} disabled={disabled} placeholder={t("settings.common.colorPlaceholder")} spellCheck={false} value={draft} onChange={(event) => { setDraft(event.target.value); setInvalid(false); }} />
              <InputGroupAddon align="inline-end"><Button type="submit" size="sm" disabled={disabled}>{t("common.actions.apply")}</Button></InputGroupAddon>
            </InputGroup>
          </form>
        </PopoverContent>
      </Popover>
    </Field>
  );
}

export function SelectRow({ label, description, disabled = false, value, options, onChange }: { label: string; description?: string; disabled?: boolean; value: string; options: Array<[string, string]>; onChange: (value: string) => void }) {
  const items = options.map(([optionValue, text]) => ({ value: optionValue, label: text }));
  return (
    <Field orientation="horizontal" className={styles.settingRow} data-disabled={disabled || undefined}>
      <FieldContent>
        <FieldTitle>{label}</FieldTitle>
        {description && <FieldDescription>{description}</FieldDescription>}
      </FieldContent>
      <Select disabled={disabled} items={items} value={value} onValueChange={(next) => { if (next !== null) onChange(next); }}>
        <SelectTrigger aria-label={label} className="w-45"><SelectValue /></SelectTrigger>
        <SelectContent><SelectGroup>{options.map(([optionValue, text]) => <SelectItem value={optionValue} key={optionValue}>{text}</SelectItem>)}</SelectGroup></SelectContent>
      </Select>
    </Field>
  );
}
