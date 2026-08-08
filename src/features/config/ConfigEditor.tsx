import { useEffect, useMemo, useRef, useState } from "react";
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
      setNotice("配置已保存并立即应用。官方注释和字段顺序已保持一致。");
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

  const defaultText = data?.defaultJsonc ?? "正在读取默认配置…";
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
        <div><h2>配置编辑器</h2><p>左右配置采用相同字段顺序和官方注释，保存时会补齐缺失字段。</p></div>
        <div className={styles.actions}>
          <button onClick={() => void reload()}>重新载入</button>
          <button disabled={!data} onClick={() => data && changeDraft(data.defaultJsonc)}>恢复默认</button>
          <button data-primary disabled={!dirty || !validation?.valid || conflict || validating || saving} onClick={() => void save()}>{saving ? "保存中…" : "保存并应用"}</button>
        </div>
      </header>

      <div className={styles.toolbar}>
        <button onClick={() => void exportConfig()}>导出配置</button>
        <button onClick={() => void api.revealConfigDirectory().catch((value) => setError(messageOf(value)))}>打开配置目录</button>
        <span data-kind={status.kind}>{status.text}</span>
      </div>

      <div className={styles.columns}>
        <section className={styles.panel}>
          <header><strong>默认配置</strong><span>只读 · 带注释</span></header>
          <div className={styles.codeFrame}>
            <pre ref={defaultLineNumbers} aria-hidden className={styles.lineNumbers}>{defaultLines}</pre>
            <pre ref={defaultEditor} aria-label="默认配置，只读" onScroll={(event) => syncScroll(event.currentTarget, userEditor.current)}><code>{defaultText}</code></pre>
          </div>
        </section>
        <section className={styles.panel} data-invalid={!validation?.valid || conflict}>
          <header><strong>我的配置</strong><span>{dirty ? "有未保存修改" : "已保存"}</span></header>
          <div className={styles.codeFrame}>
            <pre ref={userLineNumbers} aria-hidden className={styles.lineNumbers}>{userLines}</pre>
            <textarea
              ref={userEditor}
              aria-invalid={!validation?.valid || conflict}
              aria-label="我的 JSONC 配置"
              onChange={(event) => changeDraft(event.currentTarget.value)}
              onScroll={(event) => syncScroll(event.currentTarget, defaultEditor.current)}
              placeholder="在这里输入 JSONC 配置"
              spellCheck={false}
              value={draft}
            />
          </div>
        </section>
      </div>
      {!validation?.valid && <p className={styles.fallback}>右侧草稿无效：有效配置预览将整体回退到左侧默认值，运行中的应用仍保持最后一次有效配置。</p>}
    </section>
  );
}
