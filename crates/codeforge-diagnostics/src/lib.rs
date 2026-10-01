//! Normalization, deduplication, and ranking for diagnostics from multiple engines.

use std::collections::HashMap;

use codeforge_protocol::{Confidence, Diagnostic, Severity};

#[derive(Debug, Default, Clone)]
pub struct DiagnosticsAggregator {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticsAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        self.diagnostics.extend(diagnostics);
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn aggregate(self) -> Vec<Diagnostic> {
        let mut grouped = HashMap::<String, Diagnostic>::new();
        for diagnostic in self.diagnostics {
            match grouped.get_mut(&diagnostic.dedup_key()) {
                Some(existing) => merge(existing, diagnostic),
                None => {
                    grouped.insert(diagnostic.dedup_key(), diagnostic);
                }
            }
        }
        let mut diagnostics = grouped.into_values().collect::<Vec<_>>();
        diagnostics.sort_by(|left, right| {
            right
                .severity
                .rank()
                .cmp(&left.severity.rank())
                .then_with(|| left.file.cmp(&right.file))
                .then_with(|| left.range.start_byte.cmp(&right.range.start_byte))
                .then_with(|| left.rule_id.cmp(&right.rule_id))
        });
        diagnostics
    }

    pub fn into_inner(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

fn merge(existing: &mut Diagnostic, incoming: Diagnostic) {
    let confidence_rank = incoming.confidence.cmp(&existing.confidence);
    if incoming.severity.rank() > existing.severity.rank() {
        existing.severity = incoming.severity;
    }
    if confidence_rank == std::cmp::Ordering::Greater {
        existing.confidence = incoming.confidence;
    }
    if !existing
        .engine
        .split(',')
        .any(|engine| engine.trim() == incoming.engine)
    {
        existing.engine = format!("{}, {}", existing.engine, incoming.engine);
    }
    for fix in incoming.fixes {
        if !existing
            .fixes
            .iter()
            .any(|candidate| candidate.title == fix.title)
        {
            existing.fixes.push(fix);
        }
    }
    if existing.explanation.is_none() {
        existing.explanation = incoming.explanation;
    }
    if existing.source.is_none() {
        existing.source = incoming.source;
    }
    for tag in incoming.tags {
        if !existing.tags.contains(&tag) {
            existing.tags.push(tag);
        }
    }
}

pub fn normalize_severity(raw: &str) -> Severity {
    match raw.trim().to_ascii_lowercase().as_str() {
        "fatal" | "error" | "err" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        "info" | "note" | "notice" => Severity::Info,
        _ => Severity::Hint,
    }
}

pub fn normalize_confidence(raw: &str) -> Confidence {
    match raw.trim().to_ascii_lowercase().as_str() {
        "high" | "definite" | "certain" => Confidence::High,
        "low" | "weak" | "speculative" => Confidence::Low,
        _ => Confidence::Medium,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeforge_protocol::{DiagnosticCategory, Language, Severity, SourceRange};
    use std::path::PathBuf;

    fn diagnostic(engine: &str, severity: Severity) -> Diagnostic {
        Diagnostic::new(
            engine,
            Language::Python,
            "PY-001",
            severity,
            DiagnosticCategory::Correctness,
            Confidence::Medium,
            PathBuf::from("example.py"),
            SourceRange::new(0, 1, 1, 1, 1, 2).expect("range"),
            "same finding",
        )
    }

    #[test]
    fn merges_sources_and_keeps_highest_severity() {
        let mut aggregator = DiagnosticsAggregator::new();
        aggregator.push(diagnostic("ruff", Severity::Warning));
        aggregator.push(diagnostic("tree-sitter", Severity::Error));
        let results = aggregator.aggregate();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, Severity::Error);
        assert!(results[0].engine.contains("ruff"));
        assert!(results[0].engine.contains("tree-sitter"));
    }
}
