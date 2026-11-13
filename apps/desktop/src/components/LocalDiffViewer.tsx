import { useEffect, useRef } from "react";
import { monaco } from "../lib/monaco";

export default function LocalDiffViewer({ original, modified, language, theme }: {
  original: string;
  modified: string;
  language: string;
  theme: "vs" | "vs-dark";
}) {
  const container = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!container.current) return;
    const before = monaco.editor.createModel(original, language);
    const after = monaco.editor.createModel(modified, language);
    const viewer = monaco.editor.createDiffEditor(container.current, {
      theme,
      readOnly: true,
      renderSideBySide: true,
      minimap: { enabled: false },
      fontFamily: "'Cascadia Code', 'JetBrains Mono', monospace",
      fontSize: 13,
      lineHeight: 21,
      scrollBeyondLastLine: false,
      automaticLayout: true
    });
    viewer.setModel({ original: before, modified: after });
    return () => {
      // Monaco 0.57 must detach the diff model before either text model is
      // disposed. Release each owned resource when leaving/changing preview.
      viewer.setModel(null);
      viewer.dispose();
      before.dispose();
      after.dispose();
    };
  }, [original, modified, language, theme]);
  return <div ref={container} style={{ height: "100%", width: "100%", minHeight: 0 }} />;
}
