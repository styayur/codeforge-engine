use std::path::{Path, PathBuf};
use std::time::Duration;

use codeforge_engines::{find_executable, find_executable_in_workspace};
use codeforge_protocol::{
    Diagnostic, DiagnosticCategory, Language, Severity, SourceContext, SourceRange,
    ToolAdapterDescriptor, ToolCapability,
};
use codeforge_verification::{CommandSpec, ProcessResult, VerificationPipeline};

use crate::FleetError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOperation {
    FormatCheck,
    FormatApply,
    Lint,
    Verify,
    Test,
    Build,
    Benchmark,
}

impl ToolOperation {
    pub const fn label(self) -> &'static str {
        match self {
            Self::FormatCheck => "format_check",
            Self::FormatApply => "format",
            Self::Lint => "lint",
            Self::Verify => "verify",
            Self::Test => "test",
            Self::Build => "build",
            Self::Benchmark => "benchmark",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
    pub check_only: bool,
}

impl ToolContext {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            files: Vec::new(),
            check_only: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolPlan {
    pub adapter_id: String,
    pub language: Language,
    pub operation: ToolOperation,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub description: String,
}

impl ToolPlan {
    pub fn command_spec(&self) -> CommandSpec {
        let (program, args) = self
            .command
            .split_first()
            .map(|(program, args)| (program.clone(), args.to_vec()))
            .unwrap_or_else(|| (String::new(), Vec::new()));
        CommandSpec::new(program, args, &self.cwd)
            .label(self.adapter_id.clone())
            .timeout(self.timeout)
    }
}

#[derive(Debug, Clone)]
pub struct ToolExecution {
    pub plan: ToolPlan,
    pub process: ProcessResult,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct ToolAdapter {
    spec: &'static ToolSpec,
    executable: Option<PathBuf>,
}

impl ToolAdapter {
    fn discover(root: &Path, spec: &'static ToolSpec) -> Self {
        let executable = find_executable_in_workspace(root, spec.executable)
            .or_else(|| find_executable(spec.executable));
        Self { spec, executable }
    }

    pub fn descriptor(&self) -> ToolAdapterDescriptor {
        ToolAdapterDescriptor {
            id: self.spec.id.to_owned(),
            name: self.spec.name.to_owned(),
            languages: self.spec.languages.to_vec(),
            capabilities: self.spec.capabilities.clone(),
            available: self.executable.is_some(),
            version: None,
            executable: self.executable.clone(),
            reason: self
                .executable
                .is_none()
                .then(|| format!("{} was not found locally", self.spec.executable)),
        }
    }

    pub fn id(&self) -> &'static str {
        self.spec.id
    }

    pub fn supports(&self, language: Language, operation: ToolOperation) -> bool {
        self.spec.languages.contains(&language)
            && self.command_for(operation).is_some()
            && self.executable.is_some()
    }

    pub fn plan(
        &self,
        language: Language,
        operation: ToolOperation,
        context: &ToolContext,
    ) -> Option<ToolPlan> {
        if !self.supports(language, operation) {
            return None;
        }
        let executable = self.executable.as_ref()?;
        let template = self.command_for(operation)?;
        let mut command = vec![executable.to_string_lossy().into_owned()];
        command.extend(self.spec.prefix_args.iter().map(|arg| (*arg).to_owned()));
        command.extend(template.iter().map(|arg| (*arg).to_owned()));
        if !context.files.is_empty() {
            command.extend(
                context
                    .files
                    .iter()
                    .map(|file| file.to_string_lossy().into_owned()),
            );
        }
        Some(ToolPlan {
            adapter_id: self.spec.id.to_owned(),
            language,
            operation,
            command,
            cwd: context.root.clone(),
            timeout: operation_timeout(operation),
            description: format!("{} via {}", operation.label(), self.spec.name),
        })
    }

    fn command_for(&self, operation: ToolOperation) -> Option<&'static [&'static str]> {
        match operation {
            ToolOperation::FormatCheck => self.spec.format_check,
            ToolOperation::FormatApply => self.spec.format_apply,
            ToolOperation::Lint => self.spec.lint,
            ToolOperation::Verify => self.spec.verify,
            ToolOperation::Test => self.spec.test,
            ToolOperation::Build => self.spec.build,
            ToolOperation::Benchmark => self.spec.benchmark,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    adapters: Vec<ToolAdapter>,
}

impl ToolRegistry {
    pub fn discover(root: &Path, hints: &[String]) -> Self {
        let adapters = tool_specs()
            .iter()
            .filter(|spec| {
                hints.is_empty()
                    || hints
                        .iter()
                        .any(|hint| hint == spec.id || hint == spec.executable)
            })
            .map(|spec| ToolAdapter::discover(root, spec))
            .collect();
        Self { adapters }
    }

    pub fn with_all_builtins(root: &Path) -> Self {
        Self::discover(root, &[])
    }

    pub fn descriptors(&self) -> Vec<ToolAdapterDescriptor> {
        self.adapters.iter().map(ToolAdapter::descriptor).collect()
    }

    pub fn available(&self) -> Vec<&ToolAdapter> {
        self.adapters
            .iter()
            .filter(|adapter| adapter.executable.is_some())
            .collect()
    }

    pub fn plan(
        &self,
        language: Language,
        operation: ToolOperation,
        context: &ToolContext,
    ) -> Option<ToolPlan> {
        self.adapters
            .iter()
            .find(|adapter| adapter.supports(language, operation))
            .and_then(|adapter| adapter.plan(language, operation, context))
    }

    pub fn plans_for_language(
        &self,
        language: Language,
        operation: ToolOperation,
        context: &ToolContext,
    ) -> Vec<ToolPlan> {
        self.adapters
            .iter()
            .filter_map(|adapter| adapter.plan(language, operation, context))
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolRunner {
    pipeline: VerificationPipeline,
}

impl ToolRunner {
    pub async fn execute(&self, plan: ToolPlan) -> Result<ToolExecution, FleetError> {
        let process = self.pipeline.run_command(plan.command_spec()).await?;
        let diagnostics = parse_tool_output(&plan, &process);
        Ok(ToolExecution {
            plan,
            process,
            diagnostics,
        })
    }

    pub async fn version(&self, adapter: &ToolAdapter) -> Option<String> {
        let executable = adapter.executable.as_ref()?;
        let spec = CommandSpec::new(
            executable,
            ["--version"],
            executable.parent().unwrap_or_else(|| Path::new(".")),
        )
        .timeout(Duration::from_secs(10));
        let result = self.pipeline.run_command(spec).await.ok()?;
        result.success().then(|| {
            result
                .stdout
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned()
        })
    }
}

fn parse_tool_output(plan: &ToolPlan, process: &ProcessResult) -> Vec<Diagnostic> {
    let text = if process.stderr.trim().is_empty() {
        &process.stdout
    } else {
        &process.stderr
    };
    match plan.adapter_id.as_str() {
        "clippy" => parse_clippy_output(&plan.adapter_id, plan.language, text),
        "ruff-check" => parse_ruff_output(&plan.adapter_id, plan.language, text),
        "dart-analyze" => parse_dart_analyze_output(&plan.adapter_id, plan.language, text),
        "psscriptanalyzer" => parse_psscriptanalyzer_output(&plan.adapter_id, plan.language, text),
        _ => parse_generic_output(plan, text),
    }
}

fn parse_generic_output(plan: &ToolPlan, text: &str) -> Vec<Diagnostic> {
    text.lines()
        .filter_map(|line| parse_location_line(&plan.adapter_id, plan.language, line))
        .take(512)
        .collect()
}

fn parse_location_line(engine: &str, language: Language, line: &str) -> Option<Diagnostic> {
    let mut parts = line.splitn(4, ':');
    let file = parts.next()?.trim();
    let line_number = parts.next()?.trim().parse::<usize>().ok()?;
    let column = parts
        .next()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(1);
    let message = parts.next().unwrap_or(line).trim();
    if file.is_empty() || message.is_empty() {
        return None;
    }
    let range = SourceRange::new(0, 0, line_number, column, line_number, column).ok()?;
    Some(
        Diagnostic::new(
            engine,
            language,
            "CF-GEN-001",
            Severity::Warning,
            DiagnosticCategory::Style,
            codeforge_protocol::Confidence::Medium,
            PathBuf::from(file),
            range,
            message,
        )
        .with_source_context(SourceContext::classify(Path::new(file), ""))
        .with_evidence("structured external tool output"),
    )
}

fn parse_clippy_output(engine: &str, language: Language, text: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-message") {
            continue;
        }
        let Some(message) = value.get("message") else {
            continue;
        };
        let Some(span) = message
            .get("spans")
            .and_then(serde_json::Value::as_array)
            .and_then(|spans| {
                spans
                    .iter()
                    .find(|span| {
                        span.get("is_primary").and_then(serde_json::Value::as_bool) == Some(true)
                    })
                    .or_else(|| spans.first())
            })
        else {
            continue;
        };
        let Some(file) = span.get("file_name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let native_rule = message
            .get("code")
            .and_then(|code| code.get("code"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("clippy")
            .to_owned();
        let line_number = span
            .get("line_start")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1) as usize;
        let column = span
            .get("column_start")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1) as usize;
        let end_line = span
            .get("line_end")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(line_number as u64) as usize;
        let end_column = span
            .get("column_end")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(column as u64) as usize;
        let Ok(range) = SourceRange::new(0, 0, line_number, column, end_line, end_column) else {
            continue;
        };
        diagnostics.push(
            Diagnostic::new(
                engine,
                language,
                native_rule.clone(),
                severity_from_text(
                    message
                        .get("level")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("warning"),
                ),
                DiagnosticCategory::Correctness,
                codeforge_protocol::Confidence::High,
                PathBuf::from(file),
                range,
                message
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Clippy diagnostic"),
            )
            .with_native_rule(native_rule)
            .with_source_context(SourceContext::classify(Path::new(file), ""))
            .with_evidence("clippy machine-readable diagnostic"),
        );
    }
    diagnostics
}

fn parse_ruff_output(engine: &str, language: Language, text: &str) -> Vec<Diagnostic> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return parse_fallback_structured(engine, language, text);
    };
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let native_rule = item.get("code")?.as_str()?.to_owned();
            let file = item.get("filename")?.as_str()?;
            let location = item.get("location")?;
            let end = item.get("end_location")?;
            let line_number = location.get("row")?.as_u64()? as usize;
            let column = location.get("column")?.as_u64()? as usize;
            let end_line = end.get("row")?.as_u64()? as usize;
            let end_column = end.get("column")?.as_u64()? as usize;
            let range = SourceRange::new(0, 0, line_number, column, end_line, end_column).ok()?;
            Some(
                Diagnostic::new(
                    engine,
                    language,
                    native_rule.clone(),
                    Severity::Warning,
                    DiagnosticCategory::Style,
                    codeforge_protocol::Confidence::High,
                    PathBuf::from(file),
                    range,
                    item.get("message")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Ruff diagnostic"),
                )
                .with_native_rule(native_rule)
                .with_source_context(SourceContext::classify(Path::new(file), ""))
                .with_evidence("ruff machine-readable diagnostic"),
            )
        })
        .collect()
}

fn parse_dart_analyze_output(engine: &str, language: Language, text: &str) -> Vec<Diagnostic> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return parse_fallback_structured(engine, language, text);
    };
    let diagnostics = value
        .get("diagnostics")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    diagnostics
        .iter()
        .filter_map(|item| {
            let location = item.get("location")?;
            let range_value = location.get("range")?;
            let start = range_value.get("start")?;
            let end = range_value.get("end")?;
            let line_number = start.get("line")?.as_u64()? as usize;
            let column = start.get("column")?.as_u64()? as usize;
            let end_line = end.get("line")?.as_u64()? as usize;
            let end_column = end.get("column")?.as_u64()? as usize;
            let range = SourceRange::new(0, 0, line_number, column, end_line, end_column).ok()?;
            let native_rule = item
                .get("code")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("dart-analyze")
                .to_owned();
            Some(
                Diagnostic::new(
                    engine,
                    language,
                    native_rule.clone(),
                    severity_from_text(
                        item.get("severity")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("warning"),
                    ),
                    DiagnosticCategory::Style,
                    codeforge_protocol::Confidence::High,
                    PathBuf::from(
                        location
                            .get("file")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(""),
                    ),
                    range,
                    item.get("problemMessage")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("Dart analyzer diagnostic"),
                )
                .with_native_rule(native_rule)
                .with_source_context(SourceContext::classify(
                    Path::new(
                        location
                            .get("file")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(""),
                    ),
                    "",
                ))
                .with_evidence("dart analyze machine-readable diagnostic"),
            )
        })
        .collect()
}

fn parse_psscriptanalyzer_output(engine: &str, language: Language, text: &str) -> Vec<Diagnostic> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return parse_fallback_structured(engine, language, text);
    };
    let items = value.as_array().cloned().unwrap_or_else(|| vec![value]);
    items
        .iter()
        .filter_map(|item| {
            let native_rule = item.get("RuleName")?.as_str()?.to_owned();
            let line_number = item.get("Line")?.as_u64()? as usize;
            let column = item
                .get("Column")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1) as usize;
            let range = SourceRange::new(0, 0, line_number, column, line_number, column).ok()?;
            Some(
                Diagnostic::new(
                    engine,
                    language,
                    native_rule.clone(),
                    severity_from_text(
                        item.get("Severity")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("warning"),
                    ),
                    DiagnosticCategory::Style,
                    codeforge_protocol::Confidence::High,
                    PathBuf::from(
                        item.get("ScriptPath")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(""),
                    ),
                    range,
                    item.get("Message")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("PSScriptAnalyzer diagnostic"),
                )
                .with_native_rule(native_rule)
                .with_source_context(SourceContext::classify(
                    Path::new(
                        item.get("ScriptPath")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or(""),
                    ),
                    "",
                ))
                .with_evidence("PSScriptAnalyzer structured diagnostic"),
            )
        })
        .collect()
}

fn parse_fallback_structured(engine: &str, language: Language, text: &str) -> Vec<Diagnostic> {
    text.lines()
        .filter_map(|line| parse_location_line(engine, language, line))
        .take(512)
        .collect()
}

fn severity_from_text(value: &str) -> Severity {
    match value.to_ascii_lowercase().as_str() {
        "error" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        "info" | "information" | "note" => Severity::Info,
        _ => Severity::Hint,
    }
}

fn operation_timeout(operation: ToolOperation) -> Duration {
    match operation {
        ToolOperation::FormatCheck | ToolOperation::FormatApply => Duration::from_secs(60),
        ToolOperation::Lint => Duration::from_secs(120),
        ToolOperation::Verify => Duration::from_secs(300),
        ToolOperation::Test => Duration::from_secs(600),
        ToolOperation::Build => Duration::from_secs(300),
        ToolOperation::Benchmark => Duration::from_secs(900),
    }
}

#[derive(Debug)]
struct ToolSpec {
    id: &'static str,
    name: &'static str,
    executable: &'static str,
    prefix_args: &'static [&'static str],
    languages: &'static [Language],
    capabilities: ToolCapability,
    format_check: Option<&'static [&'static str]>,
    format_apply: Option<&'static [&'static str]>,
    lint: Option<&'static [&'static str]>,
    verify: Option<&'static [&'static str]>,
    test: Option<&'static [&'static str]>,
    build: Option<&'static [&'static str]>,
    benchmark: Option<&'static [&'static str]>,
}

fn capability(
    parse: bool,
    format: bool,
    lint: bool,
    fix: bool,
    verify: bool,
    benchmark: bool,
    profile: bool,
) -> ToolCapability {
    ToolCapability {
        parse,
        format,
        lint,
        fix,
        verify,
        benchmark,
        profile,
    }
}

fn tool_specs() -> &'static [ToolSpec] {
    static SPECS: std::sync::OnceLock<Vec<ToolSpec>> = std::sync::OnceLock::new();
    SPECS.get_or_init(|| {
        vec![
            spec(
                "rustfmt",
                "rustfmt",
                "rustfmt",
                &[],
                &[Language::Rust],
                capability(false, true, false, true, false, false, false),
                Some(&["--check"]),
                Some(&[]),
                None,
                None,
                None,
                None,
                None,
            ),
            spec(
                "clippy",
                "Clippy",
                "cargo",
                &[],
                &[Language::Rust],
                capability(false, false, true, false, true, false, false),
                None,
                None,
                Some(&[
                    "clippy",
                    "--message-format=json",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--",
                    "-D",
                    "warnings",
                ]),
                Some(&["check", "--workspace", "--all-targets", "--all-features"]),
                Some(&["test", "--workspace"]),
                Some(&["build", "--workspace"]),
                Some(&["bench", "--quiet"]),
            ),
            spec(
                "ruff-format",
                "Ruff formatter",
                "ruff",
                &[],
                &[Language::Python],
                capability(false, true, false, true, false, false, false),
                Some(&["format", "--check"]),
                Some(&["format"]),
                None,
                None,
                None,
                None,
                None,
            ),
            spec(
                "ruff-check",
                "Ruff linter",
                "ruff",
                &[],
                &[Language::Python],
                capability(false, false, true, true, true, false, false),
                None,
                None,
                Some(&["check", "--output-format=json", "--no-fix"]),
                Some(&["check"]),
                Some(&["test"]),
                None,
                None,
            ),
            spec(
                "biome",
                "Biome",
                "biome",
                &[],
                &[
                    Language::JavaScript,
                    Language::TypeScript,
                    Language::Json,
                    Language::Css,
                ],
                capability(false, true, true, true, false, false, false),
                Some(&["format", "--check"]),
                Some(&["format", "--write"]),
                Some(&["lint"]),
                None,
                None,
                None,
                None,
            ),
            spec(
                "prettier",
                "Prettier",
                "prettier",
                &[],
                &[
                    Language::JavaScript,
                    Language::TypeScript,
                    Language::Markdown,
                    Language::Json,
                    Language::Yaml,
                    Language::Html,
                    Language::Css,
                ],
                capability(false, true, false, true, false, false, false),
                Some(&["--check"]),
                Some(&["--write"]),
                None,
                None,
                None,
                None,
                None,
            ),
            spec(
                "eslint",
                "ESLint",
                "eslint",
                &[],
                &[Language::JavaScript, Language::TypeScript],
                capability(false, false, true, true, false, false, false),
                None,
                None,
                Some(&["--format", "unix"]),
                None,
                None,
                None,
                None,
            ),
            spec(
                "oxlint",
                "Oxlint",
                "oxlint",
                &[],
                &[Language::JavaScript, Language::TypeScript],
                capability(false, false, true, true, false, false, false),
                None,
                None,
                Some(&["--format", "unix"]),
                None,
                None,
                None,
                None,
            ),
            spec(
                "clang-format",
                "clang-format",
                "clang-format",
                &[],
                &[Language::C],
                capability(false, true, false, true, false, false, false),
                Some(&["--dry-run", "--Werror"]),
                Some(&["-i"]),
                None,
                None,
                None,
                None,
                None,
            ),
            spec(
                "clang-tidy",
                "clang-tidy",
                "clang-tidy",
                &[],
                &[Language::C],
                capability(false, false, true, true, false, false, false),
                None,
                None,
                Some(&[]),
                None,
                None,
                None,
                None,
            ),
            spec(
                "gofmt",
                "gofmt",
                "gofmt",
                &[],
                &[Language::Go],
                capability(false, true, false, true, false, false, false),
                Some(&["-l"]),
                Some(&["-w"]),
                None,
                None,
                None,
                None,
                None,
            ),
            spec(
                "gofumpt",
                "gofumpt",
                "gofumpt",
                &[],
                &[Language::Go],
                capability(false, true, false, true, false, false, false),
                Some(&["-l"]),
                Some(&["-w"]),
                None,
                None,
                None,
                None,
                None,
            ),
            spec(
                "staticcheck",
                "Staticcheck",
                "staticcheck",
                &[],
                &[Language::Go],
                capability(false, false, true, false, true, false, false),
                None,
                None,
                Some(&["./..."]),
                Some(&["./..."]),
                Some(&["test", "./..."]),
                Some(&["build", "./..."]),
                Some(&["test", "-bench=.", "-run=^$", "./..."]),
            ),
            spec(
                "google-java-format",
                "google-java-format",
                "google-java-format",
                &[],
                &[Language::Java],
                capability(false, true, false, true, false, false, false),
                Some(&["--dry-run", "--set-exit-if-changed"]),
                Some(&["--replace"]),
                None,
                None,
                None,
                None,
                None,
            ),
            spec(
                "spotless",
                "Spotless",
                "gradle",
                &[],
                &[Language::Java],
                capability(false, true, true, true, true, false, false),
                Some(&["spotlessCheck"]),
                Some(&["spotlessApply"]),
                Some(&["check"]),
                Some(&["check"]),
                Some(&["test"]),
                Some(&["build"]),
                None,
            ),
            spec(
                "dart-format",
                "Dart formatter",
                "dart",
                &[],
                &[Language::Dart],
                capability(false, true, false, true, false, false, false),
                Some(&["format", "--output=none", "--set-exit-if-changed"]),
                Some(&["format"]),
                None,
                None,
                None,
                None,
                None,
            ),
            spec(
                "dart-analyze",
                "Dart analyzer",
                "dart",
                &[],
                &[Language::Dart],
                capability(false, false, true, false, true, false, false),
                None,
                None,
                Some(&["analyze", "--format=json"]),
                Some(&["analyze", "--format=json"]),
                None,
                None,
                None,
            ),
            spec(
                "flutter-analyze",
                "Flutter analyzer",
                "flutter",
                &[],
                &[Language::Dart],
                capability(false, false, true, false, true, false, false),
                None,
                None,
                Some(&["analyze"]),
                Some(&["analyze"]),
                Some(&["test"]),
                None,
                None,
            ),
            spec(
                "psscriptanalyzer",
                "PSScriptAnalyzer",
                "pwsh",
                &[],
                &[Language::PowerShell],
                capability(true, false, true, false, true, false, false),
                None,
                None,
                Some(&[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                        "Invoke-ScriptAnalyzer -Path . -Recurse -Severity Warning,Error | ConvertTo-Json -Compress -Depth 4",
                ]),
                Some(&[
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Get-Command -Syntax $args[0]",
                    "--",
                ]),
                Some(&["-NoProfile", "-NonInteractive", "-Command", "Invoke-Pester"]),
                None,
                None,
            ),
            spec(
                "markdownlint",
                "markdownlint",
                "markdownlint",
                &[],
                &[Language::Markdown],
                capability(false, false, true, true, false, false, false),
                None,
                None,
                Some(&[]),
                None,
                None,
                None,
                None,
            ),
        ]
    })
}

#[allow(clippy::too_many_arguments)]
const fn spec(
    id: &'static str,
    name: &'static str,
    executable: &'static str,
    prefix_args: &'static [&'static str],
    languages: &'static [Language],
    capabilities: ToolCapability,
    format_check: Option<&'static [&'static str]>,
    format_apply: Option<&'static [&'static str]>,
    lint: Option<&'static [&'static str]>,
    verify: Option<&'static [&'static str]>,
    test: Option<&'static [&'static str]>,
    build: Option<&'static [&'static str]>,
    benchmark: Option<&'static [&'static str]>,
) -> ToolSpec {
    ToolSpec {
        id,
        name,
        executable,
        prefix_args,
        languages,
        capabilities,
        format_check,
        format_apply,
        lint,
        verify,
        test,
        build,
        benchmark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_describes_all_required_tool_families() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry = ToolRegistry::with_all_builtins(temp.path());
        let ids = registry
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.id)
            .collect::<Vec<_>>();
        for expected in [
            "rustfmt",
            "ruff-format",
            "biome",
            "clang-format",
            "gofmt",
            "dart-format",
            "psscriptanalyzer",
            "markdownlint",
        ] {
            assert!(ids.iter().any(|id| id == expected), "missing {expected}");
        }
    }

    #[test]
    fn native_parsers_preserve_upstream_rule_ids() {
        let clippy = r#"{"reason":"compiler-message","message":{"level":"warning","message":"unnecessary clone","code":{"code":"clippy::redundant_clone"},"spans":[{"file_name":"src/lib.rs","is_primary":true,"line_start":10,"column_start":5,"line_end":10,"column_end":20}]}}"#;
        let clippy = parse_clippy_output("clippy", Language::Rust, clippy);
        assert_eq!(
            clippy[0].native_rule_id.as_deref(),
            Some("clippy::redundant_clone")
        );

        let ruff = r#"[{"code":"F401","message":"unused import","filename":"src/example.py","location":{"row":1,"column":1},"end_location":{"row":1,"column":10}}]"#;
        let ruff = parse_ruff_output("ruff-check", Language::Python, ruff);
        assert_eq!(ruff[0].native_rule_id.as_deref(), Some("F401"));

        let dart = r#"{"diagnostics":[{"code":"unused_import","severity":"INFO","problemMessage":"Unused import","location":{"file":"lib/example.dart","range":{"start":{"line":3,"column":1},"end":{"line":3,"column":12}}}}]}"#;
        let dart = parse_dart_analyze_output("dart-analyze", Language::Dart, dart);
        assert_eq!(dart[0].native_rule_id.as_deref(), Some("unused_import"));

        let powershell = r#"[{"RuleName":"PSAvoidUsingWriteHost","Severity":"Warning","Message":"Avoid Write-Host","ScriptPath":"scripts/example.ps1","Line":4,"Column":2}]"#;
        let powershell =
            parse_psscriptanalyzer_output("psscriptanalyzer", Language::PowerShell, powershell);
        assert_eq!(
            powershell[0].native_rule_id.as_deref(),
            Some("PSAvoidUsingWriteHost")
        );
    }
}
