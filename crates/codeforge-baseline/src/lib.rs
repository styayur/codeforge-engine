//! Reviewed finding baselines and regression-only classification.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::Utc;
use codeforge_protocol::{
    BaselineComparison, BaselineComparisonEntry, BaselineEntry, BaselineFile, BaselineMatchKind,
    Diagnostic, FindingDisposition, FindingLifecycle, FindingStatus, SourceContext,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    #[error("cannot read baseline {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse baseline {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid baseline: {0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingFingerprint {
    pub exact: String,
    pub structural: String,
    pub context: String,
    pub path: PathBuf,
    pub symbol: Option<String>,
    pub context_hash: String,
    pub source_context: SourceContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingAssessment {
    pub diagnostic_id: String,
    pub fingerprint: String,
    pub rule_id: String,
    pub path: PathBuf,
    pub match_kind: BaselineMatchKind,
    pub disposition: FindingDisposition,
    pub lifecycle: FindingLifecycle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecisionRow {
    pub rule_id: String,
    pub reviewed: usize,
    pub accepted: usize,
    pub false_positive: usize,
    pub human_review: usize,
    pub fixed: usize,
}

impl PrecisionRow {
    pub fn observed_false_positive_rate(&self) -> f64 {
        if self.reviewed == 0 {
            0.0
        } else {
            self.false_positive as f64 / self.reviewed as f64
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationReport {
    pub imported: usize,
    pub skipped_tool_gaps: usize,
    pub skipped_unknown: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BaselineStore {
    file: BaselineFile,
}

impl BaselineStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_file(file: BaselineFile) -> Result<Self, BaselineError> {
        validate_baseline(&file)?;
        Ok(Self { file })
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, BaselineError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| BaselineError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let file = serde_json::from_slice(&bytes).map_err(|source| BaselineError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_file(file)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), BaselineError> {
        validate_baseline(&self.file)?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&self.file)?)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn file(&self) -> &BaselineFile {
        &self.file
    }

    pub fn entries(&self) -> &[BaselineEntry] {
        &self.file.entries
    }

    pub fn upsert(&mut self, entry: BaselineEntry) -> Result<(), BaselineError> {
        if entry.disposition.requires_reason()
            && entry
                .reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(BaselineError::Invalid(format!(
                "{} requires a non-empty reason",
                entry.disposition
            )));
        }
        if let Some(existing) = self
            .file
            .entries
            .iter_mut()
            .find(|candidate| candidate.fingerprint == entry.fingerprint)
        {
            *existing = entry;
        } else {
            self.file.entries.push(entry);
        }
        Ok(())
    }

    pub fn prune(&mut self) -> usize {
        let before = self.file.entries.len();
        self.file.entries.retain(|entry| {
            !matches!(
                entry.lifecycle,
                FindingLifecycle::Resolved | FindingLifecycle::Stale
            )
        });
        before - self.file.entries.len()
    }

    pub fn set_disposition(
        &mut self,
        fingerprint: &str,
        disposition: FindingDisposition,
        reason: Option<String>,
    ) -> Result<bool, BaselineError> {
        if disposition.requires_reason() && reason.as_deref().is_none_or(str::is_empty) {
            return Err(BaselineError::Invalid(format!(
                "{} requires a non-empty reason",
                disposition
            )));
        }
        let Some(entry) = self
            .file
            .entries
            .iter_mut()
            .find(|entry| entry.fingerprint == fingerprint)
        else {
            return Ok(false);
        };
        entry.disposition = disposition;
        entry.reason = reason;
        entry.last_reviewed = Some(Utc::now());
        entry.lifecycle = FindingLifecycle::Active;
        Ok(true)
    }

    pub fn compare(
        &self,
        diagnostics: &[Diagnostic],
        repository_root: impl AsRef<Path>,
    ) -> BaselineComparison {
        let root = repository_root.as_ref();
        let mut comparison = BaselineComparison::default();
        let mut matched = BTreeSet::new();
        let mut assessments = Vec::new();

        for diagnostic in diagnostics {
            let fingerprint = fingerprint_diagnostic(diagnostic, root);
            let candidate = match_baseline(&self.file.entries, &fingerprint);
            match candidate {
                BaselineCandidate::Exact(entry) => {
                    matched.insert(entry.fingerprint.clone());
                    let assessment =
                        assessment_from_entry(diagnostic, entry, BaselineMatchKind::Exact);
                    push_comparison(&mut comparison, assessment.clone());
                    assessments.push(assessment);
                }
                BaselineCandidate::Structural(entry) => {
                    matched.insert(entry.fingerprint.clone());
                    let assessment =
                        assessment_from_entry(diagnostic, entry, BaselineMatchKind::Structural);
                    push_comparison(&mut comparison, assessment.clone());
                    assessments.push(assessment);
                }
                BaselineCandidate::Context(entry) => {
                    matched.insert(entry.fingerprint.clone());
                    let assessment = assessment_from_entry(
                        diagnostic,
                        entry,
                        BaselineMatchKind::ContextRelocated,
                    );
                    push_comparison(&mut comparison, assessment.clone());
                    assessments.push(assessment);
                }
                BaselineCandidate::Ambiguous => {
                    comparison.ambiguous.push(BaselineComparisonEntry {
                        fingerprint: fingerprint.exact,
                        rule_id: diagnostic.rule_id.clone(),
                        path: fingerprint.path,
                        disposition: FindingDisposition::HumanReview,
                        lifecycle: FindingLifecycle::Ambiguous,
                        match_kind: BaselineMatchKind::Ambiguous,
                    });
                }
                BaselineCandidate::New => {
                    let entry = BaselineComparisonEntry {
                        fingerprint: fingerprint.exact.clone(),
                        rule_id: diagnostic.rule_id.clone(),
                        path: fingerprint.path.clone(),
                        disposition: FindingDisposition::Unreviewed,
                        lifecycle: FindingLifecycle::Active,
                        match_kind: BaselineMatchKind::New,
                    };
                    comparison.new.push(entry);
                    assessments.push(FindingAssessment {
                        diagnostic_id: diagnostic.id.clone(),
                        fingerprint: fingerprint.exact,
                        rule_id: diagnostic.rule_id.clone(),
                        path: fingerprint.path,
                        match_kind: BaselineMatchKind::New,
                        disposition: FindingDisposition::Unreviewed,
                        lifecycle: FindingLifecycle::Active,
                    });
                }
            }
        }

        for entry in &self.file.entries {
            if matched.contains(&entry.fingerprint) {
                continue;
            }
            let lifecycle = if is_rule_version_stale(entry) {
                FindingLifecycle::Stale
            } else {
                FindingLifecycle::Resolved
            };
            let comparison_entry = BaselineComparisonEntry {
                fingerprint: entry.fingerprint.clone(),
                rule_id: entry.rule_id.clone(),
                path: entry.path.clone(),
                disposition: entry.disposition,
                lifecycle,
                match_kind: BaselineMatchKind::Known,
            };
            if lifecycle == FindingLifecycle::Stale {
                comparison.stale.push(comparison_entry);
            } else {
                comparison.resolved.push(comparison_entry);
            }
        }

        comparison
    }

    pub fn classify_diagnostics(
        &self,
        diagnostics: &[Diagnostic],
        repository_root: impl AsRef<Path>,
    ) -> Vec<FindingAssessment> {
        let root = repository_root.as_ref();
        diagnostics
            .iter()
            .map(|diagnostic| {
                let fingerprint = fingerprint_diagnostic(diagnostic, root);
                match match_baseline(&self.file.entries, &fingerprint) {
                    BaselineCandidate::Exact(entry) => {
                        assessment_from_entry(diagnostic, entry, BaselineMatchKind::Exact)
                    }
                    BaselineCandidate::Structural(entry) => {
                        assessment_from_entry(diagnostic, entry, BaselineMatchKind::Structural)
                    }
                    BaselineCandidate::Context(entry) => assessment_from_entry(
                        diagnostic,
                        entry,
                        BaselineMatchKind::ContextRelocated,
                    ),
                    BaselineCandidate::Ambiguous => FindingAssessment {
                        diagnostic_id: diagnostic.id.clone(),
                        fingerprint: fingerprint.exact,
                        rule_id: diagnostic.rule_id.clone(),
                        path: fingerprint.path,
                        match_kind: BaselineMatchKind::Ambiguous,
                        disposition: FindingDisposition::HumanReview,
                        lifecycle: FindingLifecycle::Ambiguous,
                    },
                    BaselineCandidate::New => FindingAssessment {
                        diagnostic_id: diagnostic.id.clone(),
                        fingerprint: fingerprint.exact,
                        rule_id: diagnostic.rule_id.clone(),
                        path: fingerprint.path,
                        match_kind: BaselineMatchKind::New,
                        disposition: FindingDisposition::Unreviewed,
                        lifecycle: FindingLifecycle::Active,
                    },
                }
            })
            .collect()
    }

    pub fn review(
        &mut self,
        diagnosis: &Diagnostic,
        repository_root: impl AsRef<Path>,
        disposition: FindingDisposition,
        reason: Option<String>,
    ) -> Result<BaselineEntry, BaselineError> {
        let fingerprint = fingerprint_diagnostic(diagnosis, repository_root.as_ref());
        if disposition.requires_reason() && reason.as_deref().is_none_or(str::is_empty) {
            return Err(BaselineError::Invalid(format!(
                "{} requires a non-empty reason",
                disposition
            )));
        }
        let entry = BaselineEntry {
            fingerprint: fingerprint.exact,
            structural_fingerprint: Some(fingerprint.structural),
            context_fingerprint: Some(fingerprint.context),
            rule_id: diagnosis.rule_id.clone(),
            path: PathBuf::from(normalize_path(&fingerprint.path)),
            symbol: fingerprint.symbol,
            context_hash: Some(fingerprint.context_hash),
            source_context: fingerprint.source_context,
            disposition,
            reason,
            first_seen: Utc::now(),
            last_reviewed: Some(Utc::now()),
            rule_version: diagnosis.rule_version.clone(),
            lifecycle: FindingLifecycle::Active,
        };
        self.upsert(entry.clone())?;
        Ok(entry)
    }

    pub fn precision_stats(&self) -> Vec<PrecisionRow> {
        let mut rows = BTreeMap::<String, PrecisionRow>::new();
        for entry in &self.file.entries {
            let row = rows
                .entry(entry.rule_id.clone())
                .or_insert_with(|| PrecisionRow {
                    rule_id: entry.rule_id.clone(),
                    ..PrecisionRow::default()
                });
            row.reviewed += 1;
            match entry.disposition {
                FindingDisposition::Accepted => row.accepted += 1,
                FindingDisposition::FalsePositive => row.false_positive += 1,
                FindingDisposition::HumanReview => row.human_review += 1,
                FindingDisposition::Fixed => row.fixed += 1,
                FindingDisposition::Unreviewed => {}
            }
        }
        rows.into_values().collect()
    }

    pub fn migrate_legacy(
        path: impl AsRef<Path>,
    ) -> Result<(Self, MigrationReport), BaselineError> {
        let path = path.as_ref();
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        let entries = value.as_array().ok_or_else(|| {
            BaselineError::Invalid("legacy baseline must be a JSON array".to_owned())
        })?;
        let mut store = Self::new();
        let mut report = MigrationReport {
            imported: 0,
            skipped_tool_gaps: 0,
            skipped_unknown: 0,
            errors: Vec::new(),
        };
        for (index, value) in entries.iter().enumerate() {
            let classification = value
                .get("classification")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unreviewed")
                .to_ascii_lowercase();
            if classification == "tool_gap" {
                report.skipped_tool_gaps += 1;
                continue;
            }
            let disposition = match classification.as_str() {
                "accepted" => FindingDisposition::Accepted,
                "false_positive" | "false-positive" => FindingDisposition::FalsePositive,
                "human_review" | "requires_human_review" => FindingDisposition::HumanReview,
                "fixed" => FindingDisposition::Fixed,
                "unreviewed" => FindingDisposition::Unreviewed,
                _ => {
                    report.skipped_unknown += 1;
                    continue;
                }
            };
            let Some(rule_id) = value.get("rule_id").and_then(serde_json::Value::as_str) else {
                report
                    .errors
                    .push(format!("entry {index}: missing rule_id"));
                continue;
            };
            let Some(path) = value.get("file").and_then(serde_json::Value::as_str) else {
                report.errors.push(format!("entry {index}: missing file"));
                continue;
            };
            let fingerprint = value
                .get("fingerprint")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| legacy_fingerprint(rule_id, path));
            let reason = value
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let first_seen = value
                .get("accepted_at")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
            let entry = BaselineEntry {
                fingerprint,
                structural_fingerprint: None,
                context_fingerprint: None,
                rule_id: rule_id.to_owned(),
                path: PathBuf::from(normalize_path(Path::new(path))),
                symbol: value
                    .get("symbol")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                context_hash: value
                    .get("context_hash")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                source_context: SourceContext::Production,
                disposition,
                reason,
                first_seen,
                last_reviewed: None,
                rule_version: value
                    .get("codeforge_version")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                lifecycle: FindingLifecycle::Active,
            };
            if let Err(error) = store.upsert(entry) {
                report.errors.push(format!("entry {index}: {error}"));
            } else {
                report.imported += 1;
            }
        }
        Ok((store, report))
    }
}

#[derive(Debug, Clone)]
enum BaselineCandidate<'a> {
    Exact(&'a BaselineEntry),
    Structural(&'a BaselineEntry),
    Context(&'a BaselineEntry),
    Ambiguous,
    New,
}

fn match_baseline<'a>(
    entries: &'a [BaselineEntry],
    fingerprint: &FindingFingerprint,
) -> BaselineCandidate<'a> {
    if let Some(entry) = entries
        .iter()
        .find(|entry| entry.fingerprint == fingerprint.exact)
    {
        return BaselineCandidate::Exact(entry);
    }
    let structural = entries
        .iter()
        .filter(|entry| {
            entry.structural_fingerprint.as_deref() == Some(fingerprint.structural.as_str())
        })
        .collect::<Vec<_>>();
    match structural.as_slice() {
        [entry] => return BaselineCandidate::Structural(entry),
        [] => {}
        _ => return BaselineCandidate::Ambiguous,
    }
    let context = entries
        .iter()
        .filter(|entry| entry.context_fingerprint.as_deref() == Some(fingerprint.context.as_str()))
        .collect::<Vec<_>>();
    match context.as_slice() {
        [entry] => BaselineCandidate::Context(entry),
        [] => BaselineCandidate::New,
        _ => BaselineCandidate::Ambiguous,
    }
}

fn assessment_from_entry(
    diagnostic: &Diagnostic,
    entry: &BaselineEntry,
    match_kind: BaselineMatchKind,
) -> FindingAssessment {
    FindingAssessment {
        diagnostic_id: diagnostic.id.clone(),
        fingerprint: entry.fingerprint.clone(),
        rule_id: diagnostic.rule_id.clone(),
        path: entry.path.clone(),
        match_kind,
        disposition: entry.disposition,
        lifecycle: FindingLifecycle::Active,
    }
}

fn push_comparison(comparison: &mut BaselineComparison, assessment: FindingAssessment) {
    let entry = BaselineComparisonEntry {
        fingerprint: assessment.fingerprint,
        rule_id: assessment.rule_id,
        path: assessment.path,
        disposition: assessment.disposition,
        lifecycle: assessment.lifecycle,
        match_kind: assessment.match_kind,
    };
    match entry.disposition {
        FindingDisposition::Accepted => comparison.known_accepted.push(entry),
        FindingDisposition::FalsePositive => comparison.known_false_positive.push(entry),
        FindingDisposition::HumanReview => comparison.human_review.push(entry),
        FindingDisposition::Fixed | FindingDisposition::Unreviewed => comparison.new.push(entry),
    }
}

fn is_rule_version_stale(entry: &BaselineEntry) -> bool {
    entry
        .rule_version
        .as_deref()
        .is_some_and(|version| version != env!("CARGO_PKG_VERSION"))
}

pub fn fingerprint_diagnostic(
    diagnostic: &Diagnostic,
    repository_root: &Path,
) -> FindingFingerprint {
    let path = relative_path(repository_root, &diagnostic.file);
    let source = std::fs::read_to_string(if diagnostic.file.is_absolute() {
        diagnostic.file.clone()
    } else {
        repository_root.join(&path)
    })
    .ok();
    let source_context = source
        .as_deref()
        .map(|source| SourceContext::classify_at(&path, source, diagnostic.range.start_byte))
        .unwrap_or(diagnostic.source_context);
    let context = source
        .as_deref()
        .map(|source| nearby_context(source, diagnostic.range.start_line))
        .unwrap_or_default();
    let context_hash = hash_text(&normalize_whitespace(&context));
    let symbol = diagnostic.symbol.clone().or_else(|| {
        source
            .as_deref()
            .and_then(|source| extract_symbol(source, diagnostic.range.start_line))
    });
    let message = normalize_message(&diagnostic.message);
    let path_key = normalize_path(&path);
    let symbol_key = symbol.clone().unwrap_or_default();
    let exact = hash_parts(&[
        &diagnostic.rule_id,
        &path_key,
        &symbol_key,
        &message,
        &context_hash,
    ]);
    let structural = hash_parts(&[&diagnostic.rule_id, &symbol_key, &message]);
    let context = hash_parts(&[&diagnostic.rule_id, &message, &context_hash]);
    FindingFingerprint {
        exact,
        structural,
        context,
        path,
        symbol,
        context_hash,
        source_context,
    }
}

fn relative_path(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn normalize_message(message: &str) -> String {
    normalize_whitespace(&message.to_ascii_lowercase())
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn nearby_context(source: &str, line: usize) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let start = line.saturating_sub(4);
    let end = (line + 3).min(lines.len());
    if start >= end {
        return String::new();
    }
    lines[start..end]
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_symbol(source: &str, line: usize) -> Option<String> {
    let lines = source.lines().collect::<Vec<_>>();
    let line = lines.get(line.saturating_sub(1))?.trim();
    for marker in [
        "fn ",
        "def ",
        "class ",
        "function ",
        "interface ",
        "struct ",
        "enum ",
    ] {
        if let Some((_, rest)) = line.split_once(marker)
            && let Some(token) = rest
                .split(|character: char| !(character.is_alphanumeric() || character == '_'))
                .find(|token| !token.is_empty())
        {
            return Some(token.to_owned());
        }
    }
    line.split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .find(|token| !token.is_empty() && !matches!(*token, "let" | "const" | "return"))
        .map(str::to_owned)
}

fn hash_parts(parts: &[&str]) -> String {
    hash_text(&parts.join("\u{1f}"))
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn legacy_fingerprint(rule_id: &str, path: &str) -> String {
    hash_parts(&[rule_id, &normalize_path(Path::new(path)), "legacy"])
}

fn validate_baseline(file: &BaselineFile) -> Result<(), BaselineError> {
    if file.schema_version != 1 {
        return Err(BaselineError::Invalid(format!(
            "unsupported schema version {}",
            file.schema_version
        )));
    }
    for entry in &file.entries {
        if entry.rule_id.trim().is_empty() {
            return Err(BaselineError::Invalid(
                "baseline rule_id is empty".to_owned(),
            ));
        }
        if entry.path.is_absolute() {
            return Err(BaselineError::Invalid(format!(
                "baseline path must be repository-relative: {}",
                entry.path.display()
            )));
        }
        if entry.disposition.requires_reason()
            && entry
                .reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(BaselineError::Invalid(format!(
                "{} entry for {} requires a reason",
                entry.disposition, entry.rule_id
            )));
        }
    }
    Ok(())
}

pub fn finding_status(comparison: &BaselineComparison) -> FindingStatus {
    if !comparison.ambiguous.is_empty() {
        FindingStatus::Ambiguous
    } else if !comparison.new.is_empty() {
        FindingStatus::NewRegressions
    } else if comparison.known_accepted.is_empty()
        && comparison.known_false_positive.is_empty()
        && comparison.human_review.is_empty()
    {
        FindingStatus::Clean
    } else {
        FindingStatus::Findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeforge_protocol::{Confidence, DiagnosticCategory, Language, Severity, SourceRange};

    fn diagnostic(path: &Path, line: usize, message: &str) -> Diagnostic {
        Diagnostic::new(
            "test",
            Language::Rust,
            "RS-TEST-001",
            Severity::Warning,
            DiagnosticCategory::Correctness,
            Confidence::High,
            path.to_path_buf(),
            SourceRange::new(0, 1, line, 1, line, 1).expect("range"),
            message,
        )
    }

    #[test]
    fn fingerprint_is_stable_when_lines_are_inserted_before() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("src");
        std::fs::write(&path, "fn first() {\n    let value = 1;\n}\n").expect("first");
        let before = fingerprint_diagnostic(&diagnostic(&path, 2, "example"), temp.path());
        std::fs::write(
            &path,
            "// inserted\n\nfn first() {\n    let value = 1;\n}\n",
        )
        .expect("second");
        let after = fingerprint_diagnostic(&diagnostic(&path, 4, "example"), temp.path());
        assert_eq!(before.structural, after.structural);
    }

    #[test]
    fn duplicate_structural_candidates_are_ambiguous() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("src");
        std::fs::write(&path, "fn value() { let v = 1; }\n").expect("source");
        let diagnosis = diagnostic(&path, 1, "duplicate");
        let fingerprint = fingerprint_diagnostic(&diagnosis, temp.path());
        let entry = |suffix: &str| BaselineEntry {
            fingerprint: format!("other-{suffix}"),
            structural_fingerprint: Some(fingerprint.structural.clone()),
            context_fingerprint: None,
            rule_id: diagnosis.rule_id.clone(),
            path: PathBuf::from("src/lib.rs"),
            symbol: fingerprint.symbol.clone(),
            context_hash: None,
            source_context: SourceContext::Production,
            disposition: FindingDisposition::Accepted,
            reason: Some("accepted".to_owned()),
            first_seen: Utc::now(),
            last_reviewed: Some(Utc::now()),
            rule_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            lifecycle: FindingLifecycle::Active,
        };
        let store = BaselineStore::from_file(BaselineFile {
            schema_version: 1,
            codeforge_version: env!("CARGO_PKG_VERSION").to_owned(),
            entries: vec![entry("a"), entry("b")],
        })
        .expect("store");
        let comparison = store.compare(&[diagnosis], temp.path());
        assert_eq!(comparison.ambiguous.len(), 1);
    }

    #[test]
    fn migration_skips_tool_gaps_and_requires_reasons() {
        let temp = tempfile::tempdir().expect("tempdir");
        let legacy = temp.path().join("legacy.json");
        std::fs::write(
            &legacy,
            r#"[
                {"rule_id":"A","file":"a.rs","fingerprint":"f1","classification":"accepted","reason":"known debt"},
                {"rule_id":"B","file":"b.rs","classification":"tool_gap","reason":"missing tool"},
                {"rule_id":"C","file":"c.rs","classification":"false_positive"}
            ]"#,
        )
        .expect("legacy");
        let (store, report) = BaselineStore::migrate_legacy(&legacy).expect("migrate");
        assert_eq!(report.imported, 1);
        assert_eq!(report.skipped_tool_gaps, 1);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(store.entries().len(), 1);
    }

    #[test]
    fn accepted_finding_becomes_known_and_new_finding_is_regression() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("src/lib.rs");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("src");
        std::fs::write(&path, "fn value() { let value = 1; }\n").expect("source");
        let known = diagnostic(&path, 1, "known issue");
        let mut store = BaselineStore::new();
        store
            .review(
                &known,
                temp.path(),
                FindingDisposition::Accepted,
                Some("reviewed debt".to_owned()),
            )
            .expect("accept");
        let comparison = store.compare(std::slice::from_ref(&known), temp.path());
        assert_eq!(comparison.known_accepted.len(), 1);
        assert!(comparison.new.is_empty());

        let mut new = diagnostic(&path, 1, "new issue");
        new.rule_id = "RS-TEST-002".to_owned();
        let comparison = store.compare(&[known, new], temp.path());
        assert_eq!(comparison.new.len(), 1);
        assert_eq!(finding_status(&comparison), FindingStatus::NewRegressions);
    }

    #[test]
    fn structural_match_survives_file_rename() {
        let temp = tempfile::tempdir().expect("tempdir");
        let original = temp.path().join("src/original.rs");
        let renamed = temp.path().join("src/renamed.rs");
        std::fs::create_dir_all(original.parent().expect("parent")).expect("src");
        std::fs::write(&original, "fn value() { let value = 1; }\n").expect("original");
        std::fs::write(&renamed, "fn value() { let value = 1; }\n").expect("renamed");
        let original_diagnostic = diagnostic(&original, 1, "same structural issue");
        let renamed_diagnostic = diagnostic(&renamed, 1, "same structural issue");
        let mut store = BaselineStore::new();
        store
            .review(
                &original_diagnostic,
                temp.path(),
                FindingDisposition::Accepted,
                Some("reviewed".to_owned()),
            )
            .expect("accept");
        let comparison = store.compare(&[renamed_diagnostic], temp.path());
        assert_eq!(comparison.known_accepted.len(), 1);
        assert_eq!(
            comparison.known_accepted[0].match_kind,
            BaselineMatchKind::Structural
        );
    }

    #[test]
    fn changed_rule_version_marks_baseline_stale() {
        let temp = tempfile::tempdir().expect("tempdir");
        let entry = BaselineEntry {
            fingerprint: "fingerprint".to_owned(),
            structural_fingerprint: None,
            context_fingerprint: None,
            rule_id: "RS-TEST-001".to_owned(),
            path: PathBuf::from("src/lib.rs"),
            symbol: None,
            context_hash: None,
            source_context: SourceContext::Production,
            disposition: FindingDisposition::Accepted,
            reason: Some("reviewed".to_owned()),
            first_seen: Utc::now(),
            last_reviewed: Some(Utc::now()),
            rule_version: Some("old-rule-version".to_owned()),
            lifecycle: FindingLifecycle::Active,
        };
        let store = BaselineStore::from_file(BaselineFile {
            schema_version: 1,
            codeforge_version: env!("CARGO_PKG_VERSION").to_owned(),
            entries: vec![entry],
        })
        .expect("store");
        let comparison = store.compare(&[], temp.path());
        assert_eq!(comparison.stale.len(), 1);
        assert_eq!(finding_status(&comparison), FindingStatus::Clean);
    }
}
