import { useEffect, useRef, useState } from "react";
import { api, messageOf } from "../../shared/api";
import type { AppConfig, ConfigDraftValidation, ConfigEditorData } from "../../shared/types";
import { useAppConfig } from "./AppConfigProvider";
import styles from "./ConfigEditor.module.scss";

type Props = {
  onApplied: (config: AppConfig, appearanceOnly: boolean) => Promise<void>;
  setError: (message: string | null) => void;
  setNotice: (message: string | null) => void;
};

export default function ConfigEditor({ onApplied, setError, setNotice }: Props) {
  const { config, syncConfig } = useAppConfig();
  const fullImportInput = useRef<HTMLInputElement>(null);
  const appearanceImportInput = useRef<HTMLInputElement>(null);
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
      setNotice("配置已保存并立即应用；JSONC 注释已整理为标准 JSON。");
    } catch (value) {
      const message = messageOf(value);
      if (message.includes("重新载入")) setConflict(true);
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
      setNotice("配置已导出。");
    } catch (value) { setError(messageOf(value)); }
  };

  const loadFullImport = async (file?: File) => {
    if (!file) return;
    changeDraft(await file.text());
    if (fullImportInput.current) fullImportInput.current.value = "";
    setNotice("配置已载入右侧编辑器，验证通过后点击保存才会生效。");
  };

  const importAppearance = async (file?: File) => {
    if (!file) return;
    setError(null);
    try {
      const imported = await api.importAppConfig(await file.text(), true);
      dirtyRef.current = false;
      setDirty(false);
      syncConfig(imported);
      await onApplied(imported, true);
      applyEditorData(await api.getConfigEditorData());
      setNotice("桌面歌词外观已导入。");
    } catch (value) {
      setError(messageOf(value));
    } finally {
      if (appearanceImportInput.current) appearanceImportInput.current.value = "";
    }
  };

  const status = conflict
    ? { kind: "error", text: "底层配置已变化，请重新载入后继续编辑。" }
    : validating
      ? { kind: "checking", text: "正在验证 JSONC…" }
      : validation?.valid
        ? { kind: "valid", text: dirty ? "配置有效，可以保存。" : "当前配置有效。" }
        : {
            kind: "error",
            text: validation?.error
              ? `第 ${validation.error.line} 行，第 ${validation.error.column} 列：${validation.error.message}`
              : "配置无效，预览使用左侧默认配置。",
          };

  return (
    <section className={styles.editorShell}>
      <header className={styles.header}>
        <div><h2>配置编辑器</h2><p>右侧可只填写需要覆盖的字段；缺失项使用左侧默认值。</p></div>
        <div className={styles.actions}>
          <button onClick={() => void reload()}>重新载入</button>
          <button disabled={!data} onClick={() => data && changeDraft(data.defaultJsonc)}>复制默认到右侧</button>
          <button data-primary disabled={!dirty || !validation?.valid || conflict || validating || saving} onClick={() => void save()}>{saving ? "保存中…" : "保存并应用"}</button>
        </div>
      </header>

      <div className={styles.toolbar}>
        <button onClick={() => fullImportInput.current?.click()}>完整导入</button>
        <button onClick={() => appearanceImportInput.current?.click()}>仅导入外观</button>
        <button onClick={() => void exportConfig()}>导出配置</button>
        <button onClick={() => void api.revealConfigDirectory().catch((value) => setError(messageOf(value)))}>打开配置目录</button>
        <input ref={fullImportInput} hidden type="file" accept=".json,.jsonc,application/json" onChange={(event) => void loadFullImport(event.currentTarget.files?.[0])} />
        <input ref={appearanceImportInput} hidden type="file" accept=".json,.jsonc,application/json" onChange={(event) => void importAppearance(event.currentTarget.files?.[0])} />
        <span data-kind={status.kind}>{status.text}</span>
      </div>

      <div className={styles.columns}>
        <section className={styles.panel}>
          <header><strong>默认配置</strong><span>只读 · 带注释</span></header>
          <pre aria-label="默认配置，只读"><code>{data?.defaultJsonc ?? "正在读取默认配置…"}</code></pre>
        </section>
        <section className={styles.panel} data-invalid={!validation?.valid || conflict}>
          <header><strong>我的配置</strong><span>{dirty ? "有未保存修改" : "已保存"}</span></header>
          <textarea
            aria-invalid={!validation?.valid || conflict}
            aria-label="我的 JSONC 配置"
            onChange={(event) => changeDraft(event.currentTarget.value)}
            placeholder="在这里输入 JSONC 配置"
            spellCheck={false}
            value={draft}
          />
        </section>
      </div>
      {!validation?.valid && <p className={styles.fallback}>右侧草稿无效：有效配置预览将整体回退到左侧默认值，运行中的应用仍保持最后一次有效配置。</p>}
    </section>
  );
}
