//! Stable data contracts shared by the daemon, CLI, desktop app, and engine workers.
//!
//! The protocol deliberately does not unify language ASTs. It only standardizes
//! workspace metadata, diagnostics, transformations, verification, benchmarks,
//! tasks, engine capabilities, and plugin identity.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Result<T, E = ProtocolError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),
    #[error("invalid source range: start {start} is after end {end}")]
    InvalidRange { start: usize, end: usize },
    #[error("invalid SARIF document: {0}")]
    InvalidSarif(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Python,
    Rust,
    C,
    JavaScript,
    TypeScript,
    Java,
    Go,
}

impl Language {
    pub const ALL: [Self; 7] = [
        Self::Python,
        Self::Rust,
        Self::C,
        Self::JavaScript,
        Self::TypeScript,
        Self::Java,
        Self::Go,
    ];

    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "py" | "pyi" => Some(Self::Python),
            "rs" => Some(Self::Rust),
            "c" | "h" => Some(Self::C),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            "java" => Some(Self::Java),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Result<Self> {
        match name.to_ascii_lowercase().as_str() {
            "python" | "py" => Ok(Self::Python),
            "rust" | "rs" => Ok(Self::Rust),
            "c" | "clang" => Ok(Self::C),
            "javascript" | "js" | "jsx" => Ok(Self::JavaScript),
            "typescript" | "ts" | "tsx" => Ok(Self::TypeScript),
            "java" => Ok(Self::Java),
            "go" | "golang" => Ok(Self::Go),
            other => Err(ProtocolError::UnsupportedLanguage(other.to_owned())),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Rust => "rust",
            Self::C => "c",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Java => "java",
            Self::Go => "go",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Python => "Python",
            Self::Rust => "Rust",
            Self::C => "C",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Java => "Java",
            Self::Go => "Go",
        }
    }

    pub fn source_glob(self) -> &'static str {
        match self {
            Self::Python => "**/*.py",
            Self::Rust => "**/*.rs",
            Self::C => "**/*.{c,h}",
            Self::JavaScript => "**/*.{js,mjs,cjs,jsx}",
            Self::TypeScript => "**/*.{ts,tsx,mts,cts}",
            Self::Java => "**/*.java",
            Self::Go => "**/*.go",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
}

impl Severity {
    pub const fn rank(self) -> u8 {
        match self {
            Self::Hint => 0,
            Self::Info => 1,
            Self::Warning => 2,
            Self::Error => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCategory {
    Correctness,
    Performance,
    Security,
    Maintainability,
    Complexity,
    Style,
    DeadCode,
    ApiMisuse,
    Concurrency,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl SourceRange {
    pub fn new(
        start_byte: usize,
        end_byte: usize,
        start_line: usize,
        start_column: usize,
        end_line: usize,
        end_column: usize,
    ) -> Result<Self> {
        if start_byte > end_byte {
            return Err(ProtocolError::InvalidRange {
                start: start_byte,
                end: end_byte,
            });
        }
        Ok(Self {
            start_byte,
            end_byte,
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }

    pub fn from_offsets(source: &str, start_byte: usize, end_byte: usize) -> Result<Self> {
        if start_byte > end_byte || end_byte > source.len() {
            return Err(ProtocolError::InvalidRange {
                start: start_byte,
                end: end_byte,
            });
        }
        let (start_line, start_column) = line_column(source, start_byte);
        let (end_line, end_column) = line_column(source, end_byte);
        Self::new(
            start_byte,
            end_byte,
            start_line,
            start_column,
            end_line,
            end_column,
        )
    }
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let bounded = offset.min(source.len());
    let prefix = &source[..bounded];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let column = prefix[line_start..].chars().count() + 1;
    (line, column)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: SourceRange,
    pub replacement: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    pub id: String,
    pub title: String,
    pub edits: Vec<TextEdit>,
    #[serde(default)]
    pub safe: bool,
}

impl Fix {
    pub fn new(title: impl Into<String>, edits: Vec<TextEdit>, safe: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            edits,
            safe,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub id: String,
    pub engine: String,
    pub language: Language,
    pub rule_id: String,
    pub severity: Severity,
    pub category: DiagnosticCategory,
    pub confidence: Confidence,
    pub file: PathBuf,
    pub range: SourceRange,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub fixes: Vec<Fix>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Diagnostic {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        engine: impl Into<String>,
        language: Language,
        rule_id: impl Into<String>,
        severity: Severity,
        category: DiagnosticCategory,
        confidence: Confidence,
        file: PathBuf,
        range: SourceRange,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            engine: engine.into(),
            language,
            rule_id: rule_id.into(),
            severity,
            category,
            confidence,
            file,
            range,
            message: message.into(),
            explanation: None,
            source: None,
            fixes: Vec::new(),
            tags: Vec::new(),
        }
    }

    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fixes.push(fix);
        self
    }

    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    pub fn dedup_key(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}",
            self.language,
            self.file.to_string_lossy(),
            self.range.start_byte,
            self.rule_id,
            self.message.to_ascii_lowercase()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEdit {
    pub file: PathBuf,
    pub edits: Vec<TextEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transformation {
    pub id: String,
    pub engine: String,
    pub language: Language,
    pub title: String,
    pub description: String,
    pub files: Vec<PathBuf>,
    pub edits: Vec<FileEdit>,
    #[serde(default)]
    pub preconditions: Vec<String>,
    pub risk_level: RiskLevel,
    pub reversible: bool,
}

impl Transformation {
    pub fn new(
        engine: impl Into<String>,
        language: Language,
        title: impl Into<String>,
        description: impl Into<String>,
        edits: Vec<FileEdit>,
    ) -> Self {
        let files = edits.iter().map(|edit| edit.file.clone()).collect();
        Self {
            id: Uuid::new_v4().to_string(),
            engine: engine.into(),
            language,
            title: title.into(),
            description: description.into(),
            files,
            edits,
            preconditions: Vec::new(),
            risk_level: RiskLevel::Low,
            reversible: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePatch {
    pub file: PathBuf,
    pub before_hash: String,
    pub after_hash: String,
    pub additions: usize,
    pub deletions: usize,
    pub unified_diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    pub id: String,
    pub transformation_ids: Vec<String>,
    pub files: Vec<FilePatch>,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    NotRun,
    Passed,
    Failed,
    Unavailable,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub status: VerificationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
}

impl VerificationCheck {
    pub fn not_run() -> Self {
        Self {
            status: VerificationStatus::NotRun,
            message: None,
            duration_ms: None,
            command: None,
        }
    }

    pub fn passed(message: impl Into<String>, duration_ms: u128) -> Self {
        Self {
            status: VerificationStatus::Passed,
            message: Some(message.into()),
            duration_ms: Some(duration_ms),
            command: None,
        }
    }

    pub fn failed(message: impl Into<String>, duration_ms: u128) -> Self {
        Self {
            status: VerificationStatus::Failed,
            message: Some(message.into()),
            duration_ms: Some(duration_ms),
            command: None,
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: VerificationStatus::Unavailable,
            message: Some(message.into()),
            duration_ms: None,
            command: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub syntax: VerificationCheck,
    pub typecheck: VerificationCheck,
    pub build: VerificationCheck,
    pub tests: VerificationCheck,
    pub fuzz: VerificationCheck,
    pub differential: VerificationCheck,
    pub equivalence: VerificationCheck,
    pub benchmark: VerificationCheck,
}

impl Default for VerificationResult {
    fn default() -> Self {
        Self {
            syntax: VerificationCheck::not_run(),
            typecheck: VerificationCheck::not_run(),
            build: VerificationCheck::not_run(),
            tests: VerificationCheck::not_run(),
            fuzz: VerificationCheck::not_run(),
            differential: VerificationCheck::not_run(),
            equivalence: VerificationCheck::not_run(),
            benchmark: VerificationCheck::not_run(),
        }
    }
}

impl VerificationResult {
    pub fn accepted(&self) -> bool {
        self.syntax.status == VerificationStatus::Passed
            && self.build.status != VerificationStatus::Failed
            && self.tests.status != VerificationStatus::Failed
            && self.equivalence.status != VerificationStatus::Failed
    }

    pub fn evidence_level(&self) -> EvidenceLevel {
        if self.equivalence.status == VerificationStatus::Passed {
            EvidenceLevel::Verified
        } else if self.tests.status == VerificationStatus::Passed {
            EvidenceLevel::Tested
        } else if self.build.status == VerificationStatus::Passed {
            EvidenceLevel::Compiled
        } else if self.syntax.status == VerificationStatus::Passed {
            EvidenceLevel::Heuristic
        } else {
            EvidenceLevel::Unverified
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceLevel {
    Verified,
    Tested,
    Compiled,
    Heuristic,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingSummary {
    pub unit: String,
    pub samples: Vec<f64>,
    pub mean: f64,
    pub median: f64,
    pub variance: f64,
    pub min: f64,
    pub max: f64,
}

impl TimingSummary {
    pub fn from_samples(unit: impl Into<String>, mut samples: Vec<f64>) -> Self {
        samples.sort_by(f64::total_cmp);
        let count = samples.len();
        let mean = if count == 0 {
            0.0
        } else {
            samples.iter().sum::<f64>() / count as f64
        };
        let median = if count == 0 {
            0.0
        } else if count % 2 == 1 {
            samples[count / 2]
        } else {
            (samples[count / 2 - 1] + samples[count / 2]) / 2.0
        };
        let variance = if count == 0 {
            0.0
        } else {
            samples
                .iter()
                .map(|sample| (sample - mean).powi(2))
                .sum::<f64>()
                / count as f64
        };
        Self {
            unit: unit.into(),
            min: samples.first().copied().unwrap_or_default(),
            max: samples.last().copied().unwrap_or_default(),
            samples,
            mean,
            median,
            variance,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub metric: String,
    pub before: TimingSummary,
    pub after: TimingSummary,
    pub delta_percent: f64,
    pub environment: BTreeMap<String, String>,
}

impl BenchmarkResult {
    pub fn has_real_measurements(&self) -> bool {
        !self.before.samples.is_empty() && !self.after.samples.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    pub priority: i32,
    pub status: TaskStatus,
    pub submitted_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Cancelled,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub languages: Vec<Language>,
    pub capabilities: Vec<Capability>,
    pub executable: Option<PathBuf>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Parse,
    Lint,
    Format,
    Fix,
    Refactor,
    Optimize,
    Verify,
    Benchmark,
    Profile,
}

impl Capability {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Lint => "lint",
            Self::Format => "format",
            Self::Fix => "fix",
            Self::Refactor => "refactor",
            Self::Optimize => "optimize",
            Self::Verify => "verify",
            Self::Benchmark => "benchmark",
            Self::Profile => "profile",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    ReadWorkspace,
    WriteWorkspace,
    ExecuteProcess,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub metadata: EngineMetadata,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LanguageStats {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub root: PathBuf,
    pub name: String,
    pub languages: BTreeMap<Language, LanguageStats>,
    pub project_markers: Vec<String>,
    pub git_repository: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub title: String,
    pub patch: Patch,
    pub verification: VerificationResult,
    pub benchmarks: Vec<BenchmarkResult>,
    pub files: Vec<TransactionFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionFile {
    pub path: PathBuf,
    pub before_hash: String,
    pub after_hash: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifLog {
    pub version: String,
    #[serde(rename = "$schema")]
    pub schema: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifDriver {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_version: Option<String>,
    #[serde(default)]
    pub rules: Vec<SarifRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRule {
    pub id: String,
    pub name: String,
    pub short_description: SarifMessage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_description: Option<SarifMessage>,
    #[serde(default)]
    pub properties: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifResult {
    pub rule_id: String,
    pub level: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
    #[serde(default)]
    pub properties: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<SarifFix>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifFix {
    pub description: SarifMessage,
    pub artifact_changes: Vec<SarifArtifactChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifArtifactChange {
    pub artifact_location: SarifArtifactLocation,
    pub replacements: Vec<SarifReplacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifReplacement {
    pub deleted_region: SarifRegion,
    pub inserted_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifLocation {
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifPhysicalLocation {
    pub artifact_location: SarifArtifactLocation,
    pub region: SarifRegion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifArtifactLocation {
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifRegion {
    #[serde(rename = "startLine")]
    pub start_line: usize,
    #[serde(rename = "startColumn")]
    pub start_column: usize,
    #[serde(rename = "endLine")]
    pub end_line: usize,
    #[serde(rename = "endColumn")]
    pub end_column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SarifMessage {
    pub text: String,
}

pub fn diagnostics_to_sarif(diagnostics: &[Diagnostic]) -> SarifLog {
    let mut rules = BTreeMap::<String, SarifRule>::new();
    let mut results = Vec::with_capacity(diagnostics.len());

    for diagnostic in diagnostics {
        rules.entry(diagnostic.rule_id.clone()).or_insert_with(|| {
            let mut properties = BTreeMap::new();
            properties.insert(
                "category".to_owned(),
                serde_json::Value::String(
                    format!("{:?}", diagnostic.category).to_ascii_lowercase(),
                ),
            );
            properties.insert(
                "confidence".to_owned(),
                serde_json::Value::String(
                    format!("{:?}", diagnostic.confidence).to_ascii_lowercase(),
                ),
            );
            properties.insert(
                "sourceEngine".to_owned(),
                serde_json::Value::String(diagnostic.engine.clone()),
            );
            SarifRule {
                id: diagnostic.rule_id.clone(),
                name: diagnostic.rule_id.clone(),
                short_description: SarifMessage {
                    text: diagnostic.message.clone(),
                },
                full_description: diagnostic
                    .explanation
                    .clone()
                    .map(|text| SarifMessage { text }),
                properties,
            }
        });

        let fixes = diagnostic
            .fixes
            .iter()
            .map(|fix| SarifFix {
                description: SarifMessage {
                    text: fix.title.clone(),
                },
                artifact_changes: vec![SarifArtifactChange {
                    artifact_location: SarifArtifactLocation {
                        uri: path_to_sarif_uri(&diagnostic.file),
                    },
                    replacements: fix
                        .edits
                        .iter()
                        .map(|edit| SarifReplacement {
                            deleted_region: SarifRegion {
                                start_line: edit.range.start_line,
                                start_column: edit.range.start_column,
                                end_line: edit.range.end_line,
                                end_column: edit.range.end_column,
                            },
                            inserted_content: edit.replacement.clone(),
                        })
                        .collect(),
                }],
            })
            .collect();

        let mut properties = BTreeMap::new();
        properties.insert(
            "engine".to_owned(),
            serde_json::Value::String(diagnostic.engine.clone()),
        );
        properties.insert(
            "language".to_owned(),
            serde_json::Value::String(diagnostic.language.to_string()),
        );
        properties.insert(
            "tags".to_owned(),
            serde_json::Value::Array(
                diagnostic
                    .tags
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );

        results.push(SarifResult {
            rule_id: diagnostic.rule_id.clone(),
            level: match diagnostic.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info | Severity::Hint => "note",
            }
            .to_owned(),
            message: SarifMessage {
                text: diagnostic.message.clone(),
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: path_to_sarif_uri(&diagnostic.file),
                    },
                    region: SarifRegion {
                        start_line: diagnostic.range.start_line,
                        start_column: diagnostic.range.start_column,
                        end_line: diagnostic.range.end_line,
                        end_column: diagnostic.range.end_column,
                    },
                },
            }],
            properties,
            fixes,
        });
    }

    SarifLog {
        version: "2.1.0".to_owned(),
        schema: "https://json.schemastore.org/sarif-2.1.0.json".to_owned(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "CodeForge Engine".to_owned(),
                    semantic_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                    rules: rules.into_values().collect(),
                },
            },
            results,
        }],
    }
}

pub fn sarif_to_diagnostics(log: &SarifLog, default_language: Language) -> Result<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for run in &log.runs {
        let engine = run.tool.driver.name.clone();
        for result in &run.results {
            let location = result.locations.first().ok_or_else(|| {
                ProtocolError::InvalidSarif(format!("result {} has no location", result.rule_id))
            })?;
            let region = &location.physical_location.region;
            let file = PathBuf::from(sarif_uri_to_path(
                &location.physical_location.artifact_location.uri,
            ));
            let language = result
                .properties
                .get("language")
                .and_then(serde_json::Value::as_str)
                .and_then(|value| Language::from_name(value).ok())
                .unwrap_or(default_language);
            let severity = match result.level.as_str() {
                "error" => Severity::Error,
                "warning" => Severity::Warning,
                _ => Severity::Info,
            };
            let category = result
                .properties
                .get("category")
                .and_then(serde_json::Value::as_str)
                .and_then(parse_category)
                .unwrap_or(DiagnosticCategory::Correctness);
            let confidence = result
                .properties
                .get("confidence")
                .and_then(serde_json::Value::as_str)
                .and_then(parse_confidence)
                .unwrap_or(Confidence::Medium);
            let range = SourceRange::new(
                0,
                0,
                region.start_line,
                region.start_column,
                region.end_line,
                region.end_column,
            )?;
            diagnostics.push(Diagnostic::new(
                engine.clone(),
                language,
                result.rule_id.clone(),
                severity,
                category,
                confidence,
                file,
                range,
                result.message.text.clone(),
            ));
        }
    }
    Ok(diagnostics)
}

fn parse_category(value: &str) -> Option<DiagnosticCategory> {
    Some(match value {
        "correctness" => DiagnosticCategory::Correctness,
        "performance" => DiagnosticCategory::Performance,
        "security" => DiagnosticCategory::Security,
        "maintainability" => DiagnosticCategory::Maintainability,
        "complexity" => DiagnosticCategory::Complexity,
        "style" => DiagnosticCategory::Style,
        "dead_code" => DiagnosticCategory::DeadCode,
        "api_misuse" => DiagnosticCategory::ApiMisuse,
        "concurrency" => DiagnosticCategory::Concurrency,
        "memory" => DiagnosticCategory::Memory,
        _ => return None,
    })
}

fn parse_confidence(value: &str) -> Option<Confidence> {
    Some(match value {
        "low" => Confidence::Low,
        "medium" => Confidence::Medium,
        "high" => Confidence::High,
        _ => return None,
    })
}

fn path_to_sarif_uri(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sarif_uri_to_path(uri: &str) -> String {
    uri.replace('/', std::path::MAIN_SEPARATOR_STR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_detection_handles_common_extensions() {
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("tsx"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("h"), Some(Language::C));
        assert_eq!(Language::from_extension("unknown"), None);
    }

    #[test]
    fn source_ranges_are_one_based() {
        let range = SourceRange::from_offsets("a\nbc", 2, 4).expect("range");
        assert_eq!(range.start_line, 2);
        assert_eq!(range.start_column, 1);
        assert_eq!(range.end_column, 3);
    }

    #[test]
    fn sarif_roundtrip_preserves_core_fields() {
        let range = SourceRange::new(0, 3, 1, 1, 1, 4).expect("range");
        let mut diagnostic = Diagnostic::new(
            "test",
            Language::Python,
            "PY-001",
            Severity::Warning,
            DiagnosticCategory::Correctness,
            Confidence::High,
            PathBuf::from("src/example.py"),
            range,
            "example",
        );
        diagnostic.tags.push("fixture".to_owned());
        let sarif = diagnostics_to_sarif(&[diagnostic]);
        let restored = sarif_to_diagnostics(&sarif, Language::Python).expect("restore");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].rule_id, "PY-001");
        assert_eq!(restored[0].language, Language::Python);
    }
}
