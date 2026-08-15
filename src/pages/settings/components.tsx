import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Music2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldContent, FieldDescription, FieldTitle } from "@/components/ui/field";
import { IconButton } from "@/components/ui/icon-button";
import { Input } from "@/components/ui/input";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Item, ItemActions, ItemContent, ItemGroup, ItemMedia, ItemTitle } from "@/components/ui/item";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import type { RegisteredApplication } from "@/shared/types";
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

export function PageHeader({ title, description, onReset, resetting = false }: { title: string; description?: string; onReset?: () => void; resetting?: boolean; confirming?: boolean }) {
  const { t } = useTranslation();
  return (
    <header className={styles.settingsHeading}>
      <div>
        <h1 className="text-2xl font-semibold tracking-tight text-balance">{title}</h1>
        {description && <p className="text-sm text-muted-foreground">{description}</p>}
      </div>
      {onReset && <Button variant="outline" size="sm" disabled={resetting} onClick={onReset}>{resetting ? t("common.actions.resetting") : t("common.actions.resetDefault")}</Button>}
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
        nextSection = sectionElements.at(-1)?.id ?? nextSection;
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

export function ApplicationList({ applications, icons, busy, emptyLabel, removeLabel, onRemove }: {
  applications: RegisteredApplication[];
  icons: Record<string, string>;
  busy: boolean;
  emptyLabel: string;
  removeLabel: string;
  onRemove: (bundleId: string) => void;
}) {
  if (applications.length === 0) {
    return (
      <Empty className={styles.applicationEmpty}>
        <EmptyHeader>
          <EmptyMedia variant="icon"><Music2 /></EmptyMedia>
          <EmptyTitle>{emptyLabel}</EmptyTitle>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <ItemGroup className={styles.applicationList}>
      {applications.map((application) => (
        <Item variant="muted" className={styles.applicationItem} key={application.bundleId}>
          <ItemMedia variant={icons[application.bundleId] ? "image" : "icon"}>
            {icons[application.bundleId] ? <img alt="" src={icons[application.bundleId]} /> : <Music2 />}
          </ItemMedia>
          <ItemContent className={styles.applicationContent}><ItemTitle className={styles.applicationName} title={application.name}>{application.name}</ItemTitle></ItemContent>
          <ItemActions>
            <IconButton label={`${removeLabel} ${application.name}`} tooltip={removeLabel} variant="ghost" size="icon-sm" disabled={busy} onClick={() => onRemove(application.bundleId)}><X /></IconButton>
          </ItemActions>
        </Item>
      ))}
    </ItemGroup>
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

export function RangeRow({ label, description, value, min, max, step = 1, suffix, displayValue, disabled = false, onChange }: { label: string; description?: string; value: number; min: number; max: number; step?: number; suffix: string; displayValue?: number; disabled?: boolean; onChange: (value: number) => void }) {
  return (
    <Field orientation="horizontal" className={styles.settingRow} data-disabled={disabled || undefined}>
      <FieldContent>
        <FieldTitle>{label}</FieldTitle>
        {description && <FieldDescription>{description}</FieldDescription>}
      </FieldContent>
      <div className={styles.rangeControl}>
        <Slider aria-label={label} disabled={disabled} min={min} max={max} step={step} value={value} onValueChange={onChange} />
        <output className="text-xs text-muted-foreground tabular-nums">{displayValue ?? value}{suffix}</output>
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
