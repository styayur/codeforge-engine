import { useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Filter } from "lucide-react";
import type { Diagnostic, Severity } from "../types";
import { SeverityPill } from "./StatusPill";

const severityOrder: Severity[] = ["error", "warning", "info", "hint"];

export function ProblemList({
  diagnostics,
  selectedId,
  onSelect
}: {
  diagnostics: Diagnostic[];
  selectedId?: string;
  onSelect: (id: string) => void;
}) {
  const [enabled, setEnabled] = useState<Record<Severity, boolean>>({
    error: true,
    warning: true,
    info: true,
    hint: true
  });
  const filtered = useMemo(
    () => diagnostics.filter((diagnostic) => enabled[diagnostic.severity]),
    [diagnostics, enabled]
  );
  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: filtered.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 78,
    overscan: 10
  });

  return (
    <section className="problem-panel">
      <header className="panel-heading">
        <div>
          <span className="eyebrow">Problems</span>
          <strong>{filtered.length}</strong>
        </div>
        <div className="filter-group" title="Filter by severity">
          <Filter size={14} />
          {severityOrder.map((severity) => (
            <button
              key={severity}
              className={enabled[severity] ? "filter-chip active" : "filter-chip"}
              onClick={() => setEnabled((value) => ({ ...value, [severity]: !value[severity] }))}
              aria-label={`Toggle ${severity}`}
            >
              {severity.slice(0, 1).toUpperCase()}
            </button>
          ))}
        </div>
      </header>
      <div ref={parentRef} className="problem-scroll">
        <div className="virtual-body" style={{ height: virtualizer.getTotalSize() }}>
          {virtualizer.getVirtualItems().map((item) => {
            const diagnostic = filtered[item.index];
            return (
              <button
                key={diagnostic.id}
                className={selectedId === diagnostic.id ? "problem-row selected" : "problem-row"}
                style={{ transform: `translateY(${item.start}px)`, height: item.size }}
                onClick={() => onSelect(diagnostic.id)}
              >
                <div className="problem-row-top">
                  <SeverityPill severity={diagnostic.severity} />
                  <span className="rule-id">{diagnostic.rule_id}</span>
                  {diagnostic.fixes.length > 0 && <span className="fixable">fixable</span>}
                </div>
                <strong>{diagnostic.message}</strong>
                <span className="muted">
                  {diagnostic.file}:{diagnostic.range.start_line}:{diagnostic.range.start_column}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </section>
  );
}
