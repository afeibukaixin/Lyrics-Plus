import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, errorCodeOf, messageOf } from "../../shared/api";
import type { AppConfig, ConfigDraftValidation, ConfigEditorData } from "../../shared/types";
import { useAppConfig } from "./AppConfigProvider";
import { ConfigCodeEditor, type ConfigCodeEditorHandle } from "./ConfigCodeEditor";
import styles from "./ConfigEditor.module.scss";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Field } from "@/components/ui/field";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { FactoryResetDialog, FACTORY_RESET_CONFIRMATION } from "./FactoryResetDialog";

type Props = {
  onApplied: (config: AppConfig, appearanceOnly: boolean) => Promise<void>;
  setError: (message: string | null) => void;
  setNotice: (message: string | null) => void;
};

export default function ConfigEditor({ onApplied, setError, setNotice }: Props) {
  const { t } = useTranslation();
  const { config, syncConfig } = useAppConfig();
  const userEditor = useRef<ConfigCodeEditorHandle>(null);
  const dirtyRef = useRef(false);
  const validationRequest = useRef(0);
  const [data, setData] = useState<ConfigEditorData | null>(null);
  const [draft, setDraft] = useState("");
  const [validation, setValidation] = useState<ConfigDraftValidation | null>(null);
  const [dirty, setDirty] = useState(false);
  const [conflict, setConflict] = useState(false);
  const [validating, setValidating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [factoryResetOpen, setFactoryResetOpen] = useState(false);
  const [factoryResetConfirmation, setFactoryResetConfirmation] = useState("");
  const [factoryResetting, setFactoryResetting] = useState(false);
  const [factoryResetError, setFactoryResetError] = useState<string | null>(null);
  const [showDefaultConfig, setShowDefaultConfig] = useState(false);

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

  const openFactoryReset = () => {
    setFactoryResetConfirmation("");
    setFactoryResetError(null);
    setFactoryResetOpen(true);
  };

  const closeFactoryReset = (open: boolean) => {
    if (open) {
      setFactoryResetOpen(true);
      return;
    }
    setFactoryResetOpen(false);
    setFactoryResetConfirmation("");
    setFactoryResetError(null);
  };

  const factoryReset = async () => {
    const confirmation = factoryResetConfirmation.trim();
    if (confirmation !== FACTORY_RESET_CONFIRMATION || factoryResetting) return;
    setFactoryResetting(true);
    setFactoryResetError(null);
    setError(null);
    try {
      await api.factoryResetApplication(confirmation);
    } catch (value) {
      setFactoryResetting(false);
      setFactoryResetError(messageOf(value));
    }
  };

  const locateError = () => {
    const error = validation?.error;
    if (error) userEditor.current?.focusAt(error.line, error.column);
  };

  const status = !data
    ? { kind: "checking", text: t("settings.config.loading") }
    : conflict
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

  const userConfigInvalid = validation?.valid === false || conflict;

  return (
    <section className={styles.editorShell}>
      <div className={styles.toolbar}>
        <div className={styles.utilityActions}>
          <Button variant="ghost" size="sm" disabled={!data} onClick={() => void exportConfig()}>{t("settings.config.export")}</Button>
          <Button variant="ghost" size="sm" disabled={!data} onClick={() => void api.revealConfigDirectory().catch((value) => setError(messageOf(value)))}>{t("settings.config.openDirectory")}</Button>
          <Button variant="destructive" size="sm" disabled={!data} onClick={openFactoryReset}>{t("settings.config.factoryResetAction")}</Button>
        </div>
        <div className={styles.actions}>
          <Button variant="outline" size="sm" onClick={() => void reload()}>{t("common.actions.reload")}</Button>
          <Button variant="outline" size="sm" disabled={!data} onClick={() => data && changeDraft(data.resetJsonc)}>{t("common.actions.resetDefault")}</Button>
          <Button size="sm" disabled={!dirty || !validation?.valid || conflict || validating || saving} onClick={() => void save()}>{saving ? t("settings.config.saving") : t("settings.config.saveApply")}</Button>
        </div>
      </div>

      <div className={styles.editor}>
        {!data ? (
          <Card className={cn(styles.panel, styles.loadingPanel, "gap-0 py-0")}>
            <CardHeader className="border-b pt-(--card-spacing)"><CardTitle>{t("settings.config.loading")}</CardTitle></CardHeader>
            <CardContent className="grid min-h-0 px-0" />
          </Card>
        ) : (
          <Card className={cn(styles.panel, "gap-0 py-0")} data-invalid={userConfigInvalid}>
            <div className={cn(styles.panelLayout, showDefaultConfig && styles.splitPanel)}>
              {showDefaultConfig && (
                <section className={styles.pane}>
                  <CardHeader className="border-b pt-(--card-spacing)">
                    <div className={styles.panelHeading}>
                      <CardTitle>{t("settings.config.defaultConfig")}</CardTitle>
                      <Badge variant="outline">{t("settings.config.readOnly")}</Badge>
                    </div>
                  </CardHeader>
                  <CardContent className="grid min-h-0 px-0">
                    <Field className={styles.codeFrame}>
                      <ConfigCodeEditor
                        ariaLabel={t("settings.config.defaultConfigAria")}
                        className={styles.codeEditor}
                        readOnly
                        value={data.defaultJsonc}
                      />
                    </Field>
                  </CardContent>
                </section>
              )}
              {showDefaultConfig && <Separator orientation="vertical" aria-hidden className={styles.splitSeparator} />}
              <section className={styles.pane}>
                <CardHeader className="border-b pt-(--card-spacing)">
                  <div className={styles.panelHeading}>
                    <div className={styles.panelTitle}>
                      <CardTitle>{t("settings.config.myConfig")}</CardTitle>
                      <Badge className={styles.status} variant="outline" data-kind={status.kind} aria-live="polite">
                        {status.text}
                      </Badge>
                    </div>
                    <div className={styles.paneActions}>
                      {validation?.error && (
                        <Button variant="ghost" size="sm" onClick={locateError}>
                          {t("settings.config.locateError")}
                        </Button>
                      )}
                      <label className={styles.defaultToggle}>
                        <span>{t("settings.config.defaultConfig")}</span>
                        <Switch
                          aria-label={t("settings.config.defaultConfig")}
                          checked={showDefaultConfig}
                          onCheckedChange={setShowDefaultConfig}
                        />
                      </label>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="grid min-h-0 px-0">
                  <Field data-invalid={userConfigInvalid} className={styles.codeFrame}>
                    <ConfigCodeEditor
                      ariaLabel={t("settings.config.myConfigAria")}
                      className={styles.codeEditor}
                      error={validation?.valid === false ? validation.error : null}
                      invalid={userConfigInvalid}
                      onChange={changeDraft}
                      placeholderText={t("settings.config.placeholder")}
                      ref={userEditor}
                      value={draft}
                    />
                  </Field>
                </CardContent>
              </section>
            </div>
          </Card>
        )}
      </div>
      {validation?.valid === false && <Alert variant="destructive" className={styles.fallback}><AlertDescription>{t("settings.config.fallback")}</AlertDescription></Alert>}
      <FactoryResetDialog
        confirmation={factoryResetConfirmation}
        error={factoryResetError}
        onConfirmationChange={setFactoryResetConfirmation}
        onConfirm={() => void factoryReset()}
        onOpenChange={closeFactoryReset}
        open={factoryResetOpen}
        resetting={factoryResetting}
        t={t}
      />
    </section>
  );
}
