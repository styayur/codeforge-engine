import Editor, { DiffEditor } from "@monaco-editor/react";
import { FileCode2, GitCompareArrows, ShieldCheck } from "lucide-react";
import { useAppStore } from "../store";
import type { Diagnostic, FixPreviewResponse } from "../types";
import { SeverityPill } from "./StatusPill";

const languageMap: Record<string, string> = {
  python: "python",
  rust: "rust",
  c: "c",
  javascript: "javascript",
  typescript: "typescript",
  java: "java",
  go: "go"
};

export function DiffPreview({
  diagnostic,
  preview
}: {
  diagnostic?: Diagnostic;
  preview?: FixPreviewResponse;
}) {
  const theme = useAppStore((state) => state.theme);
  const editorTheme = theme === "light" || (theme === "system" && window.matchMedia("(prefers-color-scheme: light)").matches) ? "vs" : "vs-dark";
  if (preview) {
    const file = preview.files[0];
    return (
      <section className="diff-panel">
        <header className="panel-heading">
          <div>
            <span className="eyebrow">Transformation preview</span>
            <strong>{preview.diagnostic.message}</strong>
          </div>
          <span className="diff-stat">+{preview.patch.additions} -{preview.patch.deletions}</span>
        </header>
        <div className="diff-tabs">
          <GitCompareArrows size={14} />
          <span>{file?.path}</span>
        </div>
        {file ? (
          <DiffEditor
            original={file.before}
            modified={file.after}
            language={languageMap[preview.diagnostic.language] ?? "plaintext"}
            theme={editorTheme}
            options={{
              readOnly: true,
              renderSideBySide: true,
              minimap: { enabled: false },
              fontFamily: "'Cascadia Code', 'JetBrains Mono', monospace",
              fontSize: 13,
              lineHeight: 21,
              scrollBeyondLastLine: false,
              automaticLayout: true
            }}
          />
        ) : (
          <div className="empty-state">No file preview was produced.</div>
        )}
      </section>
    );
  }

  if (!diagnostic) {
    return (
      <section className="diff-panel empty-state">
        <FileCode2 size={36} />
        <h2>Select a diagnostic</h2>
        <p>Review details, inspect a generated patch, and apply only after preview.</p>
      </section>
    );
  }

  return (
    <section className="diff-panel diagnostic-detail">
      <header className="panel-heading">
        <div>
          <span className="eyebrow">{diagnostic.rule_id}</span>
          <strong>{diagnostic.message}</strong>
        </div>
        <SeverityPill severity={diagnostic.severity} />
      </header>
      <div className="detail-grid">
        <div><span>Engine</span><strong>{diagnostic.engine}</strong></div>
        <div><span>Language</span><strong>{diagnostic.language}</strong></div>
        <div><span>Confidence</span><strong>{diagnostic.confidence}</strong></div>
        <div><span>Category</span><strong>{diagnostic.category.replace("_", " ")}</strong></div>
      </div>
      <div className="explanation">
        <ShieldCheck size={18} />
        <p>{diagnostic.explanation ?? "No extended explanation is available."}</p>
      </div>
      <div className="finding-context">
        {diagnostic.file}:{diagnostic.range.start_line}:{diagnostic.range.start_column} - {diagnostic.range.end_line}:{diagnostic.range.end_column}
      </div>
      {diagnostic.source && (
        <Editor
          height="320px"
          language={languageMap[diagnostic.language] ?? "plaintext"}
          value={diagnostic.source}
          theme={editorTheme}
          options={{ readOnly: true, minimap: { enabled: false }, lineNumbersMinChars: 3 }}
        />
      )}
    </section>
  );
}
