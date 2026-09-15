import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { javascript } from "@codemirror/lang-javascript";
import { bracketMatching, HighlightStyle, indentOnInput, indentUnit, syntaxHighlighting } from "@codemirror/language";
import { lintGutter, setDiagnosticsEffect, type Diagnostic } from "@codemirror/lint";
import { Annotation, Compartment, EditorState } from "@codemirror/state";
import { drawSelection, EditorView, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers, placeholder } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { forwardRef, useEffect, useImperativeHandle, useRef, type ForwardedRef } from "react";
import type { ConfigDraftError } from "../../shared/types";

export type ConfigCodeEditorHandle = {
  focusAt: (line: number, column: number) => void;
};

type Props = {
  value: string;
  ariaLabel: string;
  onChange?: (value: string) => void;
  readOnly?: boolean;
  invalid?: boolean;
  placeholderText?: string;
  error?: ConfigDraftError | null;
  className?: string;
};

const externalChange = Annotation.define<boolean>();

const configHighlightStyle = HighlightStyle.define([
  { tag: tags.comment, color: "var(--muted-foreground)" },
  { tag: [tags.propertyName, tags.variableName], color: "var(--syntax-property)" },
  { tag: [tags.string, tags.special(tags.string)], color: "var(--syntax-string)" },
  { tag: tags.number, color: "var(--syntax-number)" },
  { tag: [tags.bool, tags.null], color: "var(--syntax-literal)" },
  { tag: [tags.punctuation, tags.bracket], color: "var(--foreground)" },
]);

const editorTheme = EditorView.theme({
  "&": {
    height: "100%",
    backgroundColor: "transparent",
    color: "var(--foreground)",
    fontFamily: "var(--font-family-mono)",
    fontSize: "0.875rem",
  },
  ".cm-scroller": {
    overflow: "auto",
    fontFamily: "inherit",
    lineHeight: "1.25rem",
  },
  ".cm-content": {
    minHeight: "100%",
    padding: "13px",
    caretColor: "var(--primary)",
  },
  ".cm-line": { padding: "0" },
  ".cm-gutters": {
    backgroundColor: "var(--card)",
    color: "var(--muted-foreground)",
    borderRight: "1px solid var(--border)",
  },
  ".cm-gutterElement": { padding: "0 9px 0 13px" },
  ".cm-activeLineGutter": { backgroundColor: "var(--accent)" },
  ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--accent) 35%, transparent)" },
  ".cm-selectionBackground, ::selection": { backgroundColor: "color-mix(in srgb, var(--primary) 25%, transparent)" },
  ".cm-focused": { outline: "none" },
  ".cm-tooltip": {
    backgroundColor: "var(--popover)",
    color: "var(--popover-foreground)",
    border: "1px solid var(--border)",
  },
  ".cm-diagnostic-error": { borderBottom: "2px solid var(--destructive)" },
  ".cm-lintPoint-error:after": { borderBottomColor: "var(--destructive)" },
  ".cm-lintRange-error": { backgroundImage: "none", borderBottom: "2px wavy var(--destructive)" },
});

function positionAt(document: EditorState["doc"], line: number, column: number) {
  const targetLine = document.line(Math.min(Math.max(line, 1), document.lines));
  return targetLine.from + Math.min(Math.max(column - 1, 0), targetLine.length);
}

function ConfigCodeEditorImpl({
  value,
  ariaLabel,
  onChange,
  readOnly = false,
  invalid = false,
  placeholderText,
  error,
  className,
}: Props, forwardedRef: ForwardedRef<ConfigCodeEditorHandle>) {
  const hostRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const editableCompartment = useRef(new Compartment());
  const attributesCompartment = useRef(new Compartment());
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useImperativeHandle(forwardedRef, () => ({
    focusAt(line, column) {
      const view = viewRef.current;
      if (!view) return;
      const position = positionAt(view.state.doc, line, column);
      const end = Math.min(position + 1, view.state.doc.length);
      view.dispatch({
        selection: { anchor: position, head: end },
        effects: EditorView.scrollIntoView(position, { y: "center" }),
      });
      view.focus();
    },
  }), []);

  useEffect(() => {
    if (!hostRef.current) return undefined;
    const view = new EditorView({
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLineGutter(),
          highlightActiveLine(),
          drawSelection(),
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap, ...closeBracketsKeymap, indentWithTab]),
          indentUnit.of("  "),
          indentOnInput(),
          bracketMatching(),
          closeBrackets(),
          javascript({ jsx: false }),
          syntaxHighlighting(configHighlightStyle, { fallback: true }),
          lintGutter(),
          editableCompartment.current.of([
            EditorState.readOnly.of(readOnly),
            EditorView.editable.of(!readOnly),
          ]),
          attributesCompartment.current.of(EditorView.contentAttributes.of({
            "aria-label": ariaLabel,
            ...(invalid ? { "aria-invalid": "true" } : {}),
            spellcheck: "false",
            ...(readOnly ? { "aria-readonly": "true" } : {}),
          })),
          ...(placeholderText ? [placeholder(placeholderText)] : []),
          editorTheme,
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) return;
            const isExternal = update.transactions.some((transaction) => transaction.annotation(externalChange));
            if (!isExternal) onChangeRef.current?.(update.state.doc.toString());
          }),
        ],
      }),
      parent: hostRef.current,
    });
    viewRef.current = view;
    return () => {
      viewRef.current = null;
      view.destroy();
    };
  }, [ariaLabel, placeholderText]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view || view.state.doc.toString() === value) return;
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: value },
      annotations: externalChange.of(true),
    });
  }, [value]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: editableCompartment.current.reconfigure([
        EditorState.readOnly.of(readOnly),
        EditorView.editable.of(!readOnly),
      ]),
    });
  }, [readOnly]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    view.dispatch({
      effects: attributesCompartment.current.reconfigure(EditorView.contentAttributes.of({
        "aria-label": ariaLabel,
        ...(invalid ? { "aria-invalid": "true" } : {}),
        spellcheck: "false",
        ...(readOnly ? { "aria-readonly": "true" } : {}),
      })),
    });
  }, [ariaLabel, invalid, readOnly]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;
    const diagnostics: Diagnostic[] = error ? (() => {
      const position = positionAt(view.state.doc, error.line, error.column);
      const diagnostic: Diagnostic = {
        from: position,
        to: Math.min(position + 1, view.state.doc.length),
        severity: "error",
        message: error.message,
      };
      return [diagnostic];
    })() : [];
    view.dispatch({ effects: setDiagnosticsEffect.of(diagnostics) });
  }, [error]);

  return <div ref={hostRef} className={className} />;
}

export const ConfigCodeEditor = forwardRef<ConfigCodeEditorHandle, Props>(ConfigCodeEditorImpl);
ConfigCodeEditor.displayName = "ConfigCodeEditor";
