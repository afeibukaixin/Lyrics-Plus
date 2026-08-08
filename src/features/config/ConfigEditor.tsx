import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, errorCodeOf, messageOf } from "../../shared/api";
import type { AppConfig, ConfigDraftValidation, ConfigEditorData } from "../../shared/types";
import { useAppConfig } from "./AppConfigProvider";
import styles from "./ConfigEditor.module.scss";

type Props = {
  onApplied: (config: AppConfig, appearanceOnly: boolean) => Promise<void>;
  setError: (message: string | null) => void;
  setNotice: (message: string | null) => void;
};

export default function ConfigEditor({ onApplied, setError, setNotice }: Props) {
  const { t } = useTranslation();
  const { config, syncConfig } = useAppConfig();
  const defaultEditor = useRef<HTMLPreElement>(null);
  const userEditor = useRef<HTMLTextAreaElement>(null);
  const defaultLineNumbers = useRef<HTMLPreElement>(null);
  const userLineNumbers = useRef<HTMLPreElement>(null);
  const dirtyRef = useRef(false);
  const validationRequest = useRef(0);
  const [data, setData] = useState<ConfigEditorData | null>(null);
  const [draft, setDraft] = useState("");
  const [validation, setValidation] = useState<ConfigDraftValidation | null>(null);
  const [dirty, setDirty] = useState(false);
  const [conflict, setConflict] = useState(false);
  const [validating, setValidating] = useState(false);
  const [saving, setSaving] = useState(false);

  const applyEditorData = (next: ConfigEditorData) => {
    validationRequest.current += 1;
    setData(next);
    setDraft(next.userJson);
    setValidation(next.validation);
    setDirty(false);
    dirtyRef.current = false;
    setConflict(false);
    setValidating(false);
  };

  const reload = async () => {
    setError(null);
    try {
      applyEditorData(await api.getConfigEditorData());
    } catch (value) {
      setError(messageOf(value));
    }
  };

  useEffect(() => { void reload(); }, []);

  useEffect(() => {
    if (!data) return;
    void api.getConfigEditorData().then((latest) => {
      if (latest.revision === data.revision) return;
      if (dirtyRef.current) {
        setConflict(true);
      } else {
        applyEditorData(latest);
      }
    }).catch((value) => setError(messageOf(value)));
  }, [config]);

  useEffect(() => {
    if (!data || !dirty) return;
    const request = validationRequest.current;
    const timer = window.setTimeout(() => {
      void api.validateAppConfigDraft(draft)
        .then((result) => { if (request === validationRequest.current) setValidation(result); })
        .catch((value) => {
          if (request !== validationRequest.current) return;
          setValidation({
            valid: false,
            error: { message: messageOf(value), line: 1, column: 1 },
            normalizedJson: null,
            effectiveConfig: data.validation.effectiveConfig,
          });
        })
        .finally(() => { if (request === validationRequest.current) setValidating(false); });
    }, 300);
    return () => window.clearTimeout(timer);
  }, [data, dirty, draft]);

  const changeDraft = (value: string) => {
    validationRequest.current += 1;
    setDraft(value);
    setDirty(true);
    dirtyRef.current = true;
    setConflict(false);
    setValidation(null);
    setValidating(true);
  };

  const save = async () => {
    if (!data || !validation?.valid || !dirty || conflict || validating) return;
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      const saved = await api.saveAppConfigDraft(draft, data.revision);
      dirtyRef.current = false;
      setDirty(false);
      syncConfig(saved);
      await onApplied(saved, false);
      applyEditorData(await api.getConfigEditorData());
      setNotice(t("settings.config.savedNotice"));
    } catch (value) {
      const message = messageOf(value);
      if (errorCodeOf(value) === "config.conflict") setConflict(true);
      setError(message);
    } finally {
      setSaving(false);
    }
  };

  const exportConfig = async () => {
    setError(null);
    try {
      const value = await api.exportAppConfig();
      const url = URL.createObjectURL(new Blob([value.raw], { type: "application/json;charset=utf-8" }));
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = value.fileName;
      anchor.click();
      URL.revokeObjectURL(url);
      setNotice(t("settings.config.exportedNotice"));
    } catch (value) { setError(messageOf(value)); }
  };

  const defaultText = data?.defaultJsonc ?? t("settings.config.loadingDefault");
  const lineNumbersOf = (value: string) =>
    Array.from({ length: value.split("\n").length }, (_, index) => index + 1).join("\n");
  const defaultLines = useMemo(() => lineNumbersOf(defaultText), [defaultText]);
  const userLines = useMemo(() => lineNumbersOf(draft), [draft]);

  const syncScroll = (source: HTMLElement, target: HTMLElement | null) => {
    if (target) {
      if (target.scrollTop !== source.scrollTop) target.scrollTop = source.scrollTop;
      if (target.scrollLeft !== source.scrollLeft) target.scrollLeft = source.scrollLeft;
    }
    const offset = `translateY(${-source.scrollTop}px)`;
    if (defaultLineNumbers.current) defaultLineNumbers.current.style.transform = offset;
    if (userLineNumbers.current) userLineNumbers.current.style.transform = offset;
  };

  const status = conflict
    ? { kind: "error", text: t("settings.config.changed") }
    : validating
      ? { kind: "checking", text: t("settings.config.validating") }
      : validation?.valid
        ? { kind: "valid", text: dirty ? t("settings.config.validSave") : t("settings.config.validCurrent") }
        : {
            kind: "error",
            text: validation?.error
              ? t("settings.config.location", { line: validation.error.line, column: validation.error.column, message: t("errors.validation") })
              : t("settings.config.invalid"),
          };

  return (
    <section className={styles.editorShell}>
      <header className={styles.header}>
        <div><h2>{t("settings.config.title")}</h2><p>{t("settings.config.description")}</p></div>
        <div className={styles.actions}>
          <button onClick={() => void reload()}>{t("common.actions.reload")}</button>
          <button disabled={!data} onClick={() => data && changeDraft(data.defaultJsonc)}>{t("common.actions.resetDefault")}</button>
          <button data-primary disabled={!dirty || !validation?.valid || conflict || validating || saving} onClick={() => void save()}>{saving ? t("settings.config.saving") : t("settings.config.saveApply")}</button>
        </div>
      </header>

      <div className={styles.toolbar}>
        <button onClick={() => void exportConfig()}>{t("settings.config.export")}</button>
        <button onClick={() => void api.revealConfigDirectory().catch((value) => setError(messageOf(value)))}>{t("settings.config.openDirectory")}</button>
        <span data-kind={status.kind}>{status.text}</span>
      </div>

      <div className={styles.columns}>
        <section className={styles.panel}>
          <header><strong>{t("settings.config.defaultConfig")}</strong><span>{t("settings.config.readOnly")}</span></header>
          <div className={styles.codeFrame}>
            <pre ref={defaultLineNumbers} aria-hidden className={styles.lineNumbers}>{defaultLines}</pre>
            <pre ref={defaultEditor} aria-label={t("settings.config.defaultAria")} onScroll={(event) => syncScroll(event.currentTarget, userEditor.current)}><code>{defaultText}</code></pre>
          </div>
        </section>
        <section className={styles.panel} data-invalid={!validation?.valid || conflict}>
          <header><strong>{t("settings.config.myConfig")}</strong><span>{dirty ? t("settings.config.unsaved") : t("settings.config.saved")}</span></header>
          <div className={styles.codeFrame}>
            <pre ref={userLineNumbers} aria-hidden className={styles.lineNumbers}>{userLines}</pre>
            <textarea
              ref={userEditor}
              aria-invalid={!validation?.valid || conflict}
              aria-label={t("settings.config.myConfigAria")}
              onChange={(event) => changeDraft(event.currentTarget.value)}
              onScroll={(event) => syncScroll(event.currentTarget, defaultEditor.current)}
              placeholder={t("settings.config.placeholder")}
              spellCheck={false}
              value={draft}
            />
          </div>
        </section>
      </div>
      {!validation?.valid && <p className={styles.fallback}>{t("settings.config.fallback")}</p>}
    </section>
  );
}
