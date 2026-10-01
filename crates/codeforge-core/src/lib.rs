//! Shared orchestration used by both the CLI and Tauri desktop application.

mod ai;
mod config;
mod sandbox;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::Instant;

use codeforge_benchmark::BenchmarkRunner;
use codeforge_diagnostics::{DiagnosticsAggregator, normalize_severity};
use codeforge_engines::{EngineRegistry, EngineRequest, find_executable};
use codeforge_git::GitRepository;
use codeforge_protocol::{
    BenchmarkResult, Confidence, Diagnostic, DiagnosticCategory, EngineStatus, FileEdit, Language,
    Patch, SourceRange, TextEdit, TimingSummary, Transformation, VerificationCheck,
    VerificationResult, VerificationStatus, WorkspaceSummary,
};
use codeforge_transform::{ApplyOptions, PreparedChange, TransactionManager, TransformError};
use codeforge_verification::{
    CommandSpec, VerificationError, VerificationPipeline, VerificationPlan,
};
use codeforge_workspace::{WorkspaceError, WorkspaceManager};
use serde::{Deserialize, Serialize};

pub use ai::{
    AiError, AiExplanation, AiPolicy, AiProvider, AiTransformProposal, DisabledAiProvider,
};
pub use config::{CodeForgeConfig, CommandConfig};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Verification(#[from] VerificationError),
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Sandbox(#[from] sandbox::SandboxError),
    #[error("diagnostic not found: {0}")]
    DiagnosticNotFound(String),
    #[error("fix index {index} not found for diagnostic {diagnostic_id}")]
    FixNotFound { diagnostic_id: String, index: usize },
    #[error("Git operation failed: {0}")]
    Git(#[from] codeforge_git::GitError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("benchmark failed: {0}")]
    Benchmark(#[from] codeforge_benchmark::BenchmarkError),
    #[error("invalid project metadata: {0}")]
    InvalidProject(String),
}

#[derive(Debug, Clone, Default)]
pub struct ReviewOptions {
    pub languages: Vec<Language>,
    pub changed_only: bool,
    pub include_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    pub diagnostics: Vec<Diagnostic>,
    pub files_analyzed: usize,
    pub languages: Vec<Language>,
    pub engines_used: Vec<String>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone)]
pub struct FixPreview {
    pub diagnostic: Diagnostic,
    pub preview: PreparedChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSnapshot {
    pub metric: String,
    pub command: Vec<String>,
    pub summary: TimingSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationCandidate {
    pub diagnostic_id: String,
    pub rule_id: String,
    pub title: String,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationReport {
    pub candidates: Vec<OptimizationCandidate>,
    pub applied: bool,
    pub patch: Option<Patch>,
    pub verification: VerificationResult,
    pub benchmark: Option<BenchmarkResult>,
    pub message: String,
}

pub struct CodeForgeEngine {
    root: PathBuf,
    workspace: WorkspaceManager,
    registry: EngineRegistry,
    config: CodeForgeConfig,
    diagnostics: RwLock<Vec<Diagnostic>>,
    analysis_cache: RwLock<HashMap<PathBuf, CachedFileAnalysis>>,
}

#[derive(Debug, Clone)]
struct CachedFileAnalysis {
    content_hash: String,
    diagnostics: Vec<Diagnostic>,
}

impl CodeForgeEngine {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CoreError> {
        let root = root.as_ref();
        let workspace = WorkspaceManager::open(root)?;
        let root = workspace.root().to_path_buf();
        let config = CodeForgeConfig::load(&root)?;
        let mut registry = EngineRegistry::with_builtin_adapters();
        registry.extend(EngineRegistry::with_external_tools(&root));
        Ok(Self {
            root,
            workspace,
            registry,
            config,
            diagnostics: RwLock::new(Vec::new()),
            analysis_cache: RwLock::new(HashMap::new()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn summary(&self) -> WorkspaceSummary {
        let mut summary = self.workspace.summary();
        if summary.git_repository {
            summary.git_branch = git_branch_sync(&self.root);
        }
        summary
    }

    pub fn engine_statuses(&self) -> Vec<EngineStatus> {
        self.registry.statuses()
    }

    pub async fn review(&self, options: ReviewOptions) -> Result<ReviewReport, CoreError> {
        let started = Instant::now();
        let selected_languages = if options.languages.is_empty() {
            self.workspace
                .language_stats()
                .keys()
                .copied()
                .collect::<Vec<_>>()
        } else {
            options.languages.clone()
        };
        let files = self
            .selected_files(&selected_languages, options.changed_only)
            .await?;
        let (fresh_files, mut reused_diagnostics, content_hashes) =
            self.partition_cached_files(&files);
        let request = EngineRequest::new(&self.root, fresh_files);
        let mut output = self.registry.analyze(&request).await;
        if options.include_external {
            let external = self
                .run_external_review(&files, &selected_languages)
                .await?;
            output.extend(external);
        }
        output.diagnostics.append(&mut reused_diagnostics);

        let diagnostics = DiagnosticsAggregator::new()
            .tap(|aggregator| aggregator.extend(output.diagnostics))
            .aggregate();
        self.update_analysis_cache(&files, &content_hashes, &diagnostics);
        let engines_used = diagnostics
            .iter()
            .flat_map(|diagnostic| {
                diagnostic
                    .engine
                    .split(',')
                    .map(str::trim)
                    .map(str::to_owned)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if let Ok(mut cache) = self.diagnostics.write() {
            *cache = diagnostics.clone();
        }
        Ok(ReviewReport {
            diagnostics,
            files_analyzed: files.len(),
            languages: selected_languages,
            engines_used,
            duration_ms: started.elapsed().as_millis(),
        })
    }

    pub fn preview_fix(
        &self,
        diagnostic_id: &str,
        fix_index: usize,
    ) -> Result<FixPreview, CoreError> {
        let diagnostic = self
            .diagnostics
            .read()
            .ok()
            .and_then(|diagnostics| {
                diagnostics
                    .iter()
                    .find(|candidate| candidate.id == diagnostic_id)
                    .cloned()
            })
            .ok_or_else(|| CoreError::DiagnosticNotFound(diagnostic_id.to_owned()))?;
        let fix =
            diagnostic
                .fixes
                .get(fix_index)
                .cloned()
                .ok_or_else(|| CoreError::FixNotFound {
                    diagnostic_id: diagnostic_id.to_owned(),
                    index: fix_index,
                })?;
        let transformation = Transformation::new(
            diagnostic.engine.clone(),
            diagnostic.language,
            fix.title.clone(),
            diagnostic.message.clone(),
            vec![FileEdit {
                file: diagnostic.file.clone(),
                edits: fix.edits,
            }],
        );
        let preview = TransactionManager::new(&self.root)?.preview(&[transformation])?;
        Ok(FixPreview {
            diagnostic,
            preview,
        })
    }

    pub fn apply_preview(
        &self,
        preview: &PreparedChange,
        title: impl Into<String>,
        force: bool,
    ) -> Result<codeforge_protocol::TransactionRecord, CoreError> {
        let verification = verify_prepared_syntax(preview)?;
        Ok(TransactionManager::new(&self.root)?.apply(
            preview,
            title,
            verification,
            Vec::new(),
            ApplyOptions { force },
        )?)
    }

    pub fn undo(&self, transaction_id: &str) -> Result<(), CoreError> {
        TransactionManager::new(&self.root)?.undo(transaction_id)?;
        Ok(())
    }

    pub fn history(&self) -> Result<Vec<codeforge_protocol::TransactionRecord>, CoreError> {
        Ok(TransactionManager::new(&self.root)?.history()?)
    }

    pub async fn verify(&self, full: bool) -> Result<VerificationResult, CoreError> {
        self.verify_root(&self.root, full).await
    }

    pub async fn benchmark(&self, samples: usize) -> Result<BenchmarkSnapshot, CoreError> {
        let samples = samples.max(1);
        let command = self.benchmark_command(&self.root).ok_or_else(|| {
            CoreError::InvalidProject("no benchmark command is configured or detectable".to_owned())
        })?;
        let runner = BenchmarkRunner::new(1, samples)?;
        let summary = runner.measure(command.clone()).await?;
        Ok(BenchmarkSnapshot {
            metric: command.label.clone(),
            command: command.command_line(),
            summary,
        })
    }

    pub async fn optimize(&self) -> Result<OptimizationReport, CoreError> {
        let review = self
            .review(ReviewOptions {
                include_external: true,
                ..ReviewOptions::default()
            })
            .await?;
        let performance = review
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.category == DiagnosticCategory::Performance
                    && diagnostic.fixes.iter().any(|fix| fix.safe)
            })
            .cloned()
            .collect::<Vec<_>>();
        if performance.is_empty() {
            return Ok(OptimizationReport {
                candidates: Vec::new(),
                applied: false,
                patch: None,
                verification: VerificationResult::default(),
                benchmark: None,
                message:
                    "No safe performance transformation was found. No performance claim is made."
                        .to_owned(),
            });
        }

        let candidates = performance
            .iter()
            .map(|diagnostic| OptimizationCandidate {
                diagnostic_id: diagnostic.id.clone(),
                rule_id: diagnostic.rule_id.clone(),
                title: diagnostic.message.clone(),
                file: diagnostic.file.clone(),
                line: diagnostic.range.start_line,
            })
            .collect::<Vec<_>>();
        let transformations = performance
            .into_iter()
            .flat_map(|diagnostic| {
                diagnostic
                    .fixes
                    .into_iter()
                    .filter(|fix| fix.safe)
                    .map(move |fix| {
                        Transformation::new(
                            diagnostic.engine.clone(),
                            diagnostic.language,
                            fix.title,
                            diagnostic.message.clone(),
                            vec![FileEdit {
                                file: diagnostic.file.clone(),
                                edits: fix.edits,
                            }],
                        )
                    })
            })
            .collect::<Vec<_>>();

        let sandbox = sandbox::Sandbox::create(&self.root)?;
        let manager = TransactionManager::new(sandbox.root())?;
        let prepared = manager.preview(&transformations)?;
        let syntax = verify_prepared_syntax(&prepared)?;
        manager.apply(
            &prepared,
            "Verified optimization candidate",
            syntax,
            Vec::new(),
            ApplyOptions { force: false },
        )?;

        let verification = self.verify_root(sandbox.root(), true).await?;
        let benchmark = match (
            self.benchmark_command(&self.root),
            self.benchmark_command(sandbox.root()),
        ) {
            (Some(before), Some(after)) => {
                let runner = BenchmarkRunner::new(1, 3)?;
                Some(runner.compare("optimization", before, after).await?)
            }
            _ => None,
        };
        Ok(OptimizationReport {
            candidates,
            applied: true,
            patch: Some(prepared.patch.clone()),
            verification,
            benchmark,
            message: "Candidate was applied only to a temporary workspace and verified there. It was not applied to the source workspace."
                .to_owned(),
        })
    }

    pub async fn verify_root(
        &self,
        root: &Path,
        full: bool,
    ) -> Result<VerificationResult, CoreError> {
        let workspace = WorkspaceManager::open(root)?;
        let languages = workspace
            .language_stats()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let files = workspace
            .records()
            .into_iter()
            .filter(|record| record.language.is_some())
            .map(|record| record.path)
            .collect::<Vec<_>>();
        let syntax = verify_syntax_files(&files);
        let plan = self.verification_plan(root, &languages, full);
        let mut result = VerificationPipeline.run(plan).await;
        result.syntax = syntax;
        Ok(result)
    }

    fn partition_cached_files(
        &self,
        files: &[PathBuf],
    ) -> (Vec<PathBuf>, Vec<Diagnostic>, HashMap<PathBuf, String>) {
        let mut fresh = Vec::new();
        let mut cached = Vec::new();
        let mut hashes = HashMap::new();
        let cache = self
            .analysis_cache
            .read()
            .ok()
            .map(|cache| cache.clone())
            .unwrap_or_default();
        for file in files {
            let Ok(record) = self.workspace.refresh_file(file) else {
                fresh.push(file.clone());
                continue;
            };
            hashes.insert(file.clone(), record.hash.clone());
            if let Some(entry) = cache.get(file)
                && entry.content_hash == record.hash
            {
                cached.extend(entry.diagnostics.clone());
            } else {
                fresh.push(file.clone());
            }
        }
        (fresh, cached, hashes)
    }

    fn update_analysis_cache(
        &self,
        files: &[PathBuf],
        hashes: &HashMap<PathBuf, String>,
        diagnostics: &[Diagnostic],
    ) {
        if let Ok(mut cache) = self.analysis_cache.write() {
            for file in files {
                let Some(hash) = hashes.get(file) else {
                    continue;
                };
                let file_diagnostics = diagnostics
                    .iter()
                    .filter(|diagnostic| &diagnostic.file == file)
                    .cloned()
                    .collect::<Vec<_>>();
                cache.insert(
                    file.clone(),
                    CachedFileAnalysis {
                        content_hash: hash.clone(),
                        diagnostics: file_diagnostics,
                    },
                );
            }
        }
    }

    async fn selected_files(
        &self,
        languages: &[Language],
        changed_only: bool,
    ) -> Result<Vec<PathBuf>, CoreError> {
        let language_set = languages.iter().copied().collect::<HashSet<_>>();
        let mut files = self
            .workspace
            .records()
            .into_iter()
            .filter(|record| {
                record
                    .language
                    .is_some_and(|language| language_set.contains(&language))
            })
            .map(|record| record.path)
            .collect::<Vec<_>>();
        if changed_only {
            let repository = GitRepository::discover(&self.root)?;
            let changed = repository
                .changed_files()
                .await?
                .into_iter()
                .map(|file| self.root.join(file.path))
                .collect::<HashSet<_>>();
            files.retain(|file| changed.contains(file));
        }
        files.sort();
        Ok(files)
    }

    async fn run_external_review(
        &self,
        files: &[PathBuf],
        languages: &[Language],
    ) -> Result<codeforge_engines::EngineOutput, CoreError> {
        let mut output = codeforge_engines::EngineOutput::default();
        let pipeline = VerificationPipeline;
        if languages.contains(&Language::Python)
            && let Some(ruff) = find_executable("ruff")
        {
            for chunk in files.chunks(100) {
                let python_files = chunk
                    .iter()
                    .filter(|file| {
                        file.extension()
                            .and_then(|extension| extension.to_str())
                            .and_then(Language::from_extension)
                            == Some(Language::Python)
                    })
                    .map(|file| file.to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                if python_files.is_empty() {
                    continue;
                }
                let mut args = vec![
                    "check".to_owned(),
                    "--output-format=json".to_owned(),
                    "--no-fix".to_owned(),
                ];
                args.extend(python_files);
                let spec = CommandSpec::new(ruff.clone(), args, &self.root)
                    .label("ruff")
                    .timeout(std::time::Duration::from_secs(120))
                    .max_output_bytes(32 * 1024 * 1024);
                if let Ok(result) = pipeline.run_command(spec).await
                    && matches!(result.exit_code, Some(0 | 1))
                {
                    output
                        .diagnostics
                        .extend(parse_ruff_json(&result.stdout, &self.root));
                }
            }
        }

        if languages.contains(&Language::Rust)
            && self.root.join("Cargo.toml").exists()
            && let Some(cargo) = find_executable("cargo")
        {
            let spec = CommandSpec::new(
                cargo,
                [
                    "clippy",
                    "--message-format=json",
                    "--all-targets",
                    "--all-features",
                    "--",
                    "-D",
                    "warnings",
                ],
                &self.root,
            )
            .label("clippy")
            .timeout(std::time::Duration::from_secs(300))
            .max_output_bytes(64 * 1024 * 1024);
            let allowed = files.iter().cloned().collect::<HashSet<_>>();
            if let Ok(result) = pipeline.run_command(spec).await {
                output
                    .diagnostics
                    .extend(parse_clippy_json(&result.stdout, &self.root, &allowed));
            }
        }
        Ok(output)
    }

    fn verification_plan(
        &self,
        root: &Path,
        languages: &[Language],
        full: bool,
    ) -> VerificationPlan {
        let mut plan = VerificationPlan::default();
        for (id, field) in [
            ("syntax", 0usize),
            ("typecheck", 1),
            ("build", 2),
            ("test", 3),
            ("fuzz", 4),
            ("differential", 5),
            ("equivalence", 6),
            ("benchmark", 7),
        ] {
            if let Some(command) = self.config.command(root, id) {
                match field {
                    0 => plan.syntax = Some(command),
                    1 => plan.typecheck = Some(command),
                    2 => plan.build = Some(command),
                    3 => plan.tests = Some(command),
                    4 => plan.fuzz = Some(command),
                    5 => plan.differential = Some(command),
                    6 => plan.equivalence = Some(command),
                    _ => plan.benchmark = Some(command),
                }
            }
        }

        if !full {
            return plan;
        }
        for language in languages {
            match language {
                Language::Python => {
                    if let Some(python) = find_executable("python")
                        && (root.join("tests").exists() || root.join("pyproject.toml").exists())
                    {
                        plan.tests = Some(CommandSpec::new(python, ["-m", "pytest"], root));
                    }
                }
                Language::Rust => {
                    if let Some(cargo) = find_executable("cargo") {
                        plan.build = Some(CommandSpec::new(
                            cargo.clone(),
                            ["check", "--all-targets"],
                            root,
                        ));
                        plan.tests = Some(CommandSpec::new(cargo, ["test", "--all-targets"], root));
                    }
                }
                Language::JavaScript | Language::TypeScript => {
                    if let Some((program, args)) = self.package_manager_command(root) {
                        let package = read_package_json(root).ok();
                        if package
                            .as_ref()
                            .is_some_and(|value| has_script(value, "typecheck"))
                        {
                            plan.typecheck = Some(CommandSpec::new(
                                program.clone(),
                                [args.clone(), vec!["run".to_owned(), "typecheck".to_owned()]]
                                    .concat(),
                                root,
                            ));
                        }
                        if package
                            .as_ref()
                            .is_some_and(|value| has_script(value, "build"))
                        {
                            plan.build = Some(CommandSpec::new(
                                program.clone(),
                                [args.clone(), vec!["run".to_owned(), "build".to_owned()]].concat(),
                                root,
                            ));
                        }
                        if package
                            .as_ref()
                            .is_some_and(|value| has_script(value, "test"))
                        {
                            plan.tests = Some(CommandSpec::new(
                                program,
                                [args, vec!["test".to_owned()]].concat(),
                                root,
                            ));
                        }
                    }
                }
                Language::Java => {
                    if root.join("pom.xml").exists() {
                        if let Some(mvn) = find_executable("mvn") {
                            plan.tests = Some(CommandSpec::new(mvn, ["test"], root));
                        }
                    } else if (root.join("build.gradle").exists()
                        || root.join("build.gradle.kts").exists())
                        && let Some(gradle) = find_executable("gradle")
                    {
                        plan.tests = Some(CommandSpec::new(gradle, ["test"], root));
                    }
                }
                Language::Go => {
                    if let Some(go) = find_executable("go") {
                        plan.build = Some(CommandSpec::new(go.clone(), ["test", "./..."], root));
                        plan.tests = Some(CommandSpec::new(go, ["test", "./..."], root));
                    }
                }
                Language::C => {
                    if root.join("CMakeLists.txt").exists()
                        && let Some(cmake) = find_executable("cmake")
                    {
                        let build_dir = root.join(".codeforge").join("build");
                        plan.build = Some(CommandSpec::new(
                            cmake.clone(),
                            [
                                "-S",
                                root.to_string_lossy().as_ref(),
                                "-B",
                                build_dir.to_string_lossy().as_ref(),
                            ],
                            root,
                        ));
                        plan.tests = Some(CommandSpec::new(
                            cmake,
                            ["--build", build_dir.to_string_lossy().as_ref()],
                            root,
                        ));
                    }
                }
            }
        }
        plan
    }

    fn benchmark_command(&self, root: &Path) -> Option<CommandSpec> {
        if let Some(command) = self.config.command(root, "benchmark") {
            return Some(command);
        }
        if root.join("Cargo.toml").exists() {
            return find_executable("cargo").map(|cargo| {
                CommandSpec::new(cargo, ["bench", "--quiet"], root).label("cargo benchmark")
            });
        }
        if root.join("go.mod").exists() {
            return find_executable("go").map(|go| {
                CommandSpec::new(go, ["test", "-bench=.", "-run=^$", "./..."], root)
                    .label("go benchmark")
            });
        }
        if let Ok(package) = read_package_json(root)
            && (has_script(&package, "bench") || has_script(&package, "benchmark"))
        {
            let (program, mut args) = self.package_manager_command(root)?;
            let script = if has_script(&package, "bench") {
                "bench"
            } else {
                "benchmark"
            };
            args.extend(["run".to_owned(), script.to_owned()]);
            return Some(CommandSpec::new(program, args, root).label("package benchmark"));
        }
        None
    }

    fn package_manager_command(&self, root: &Path) -> Option<(PathBuf, Vec<String>)> {
        if root.join("pnpm-lock.yaml").exists() {
            find_executable("pnpm").map(|path| (path, Vec::new()))
        } else if root.join("yarn.lock").exists() {
            find_executable("yarn").map(|path| (path, Vec::new()))
        } else {
            find_executable("npm").map(|path| (path, Vec::new()))
        }
    }
}

fn parse_ruff_json(output: &str, root: &Path) -> Vec<Diagnostic> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(output) else {
        return Vec::new();
    };
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let rule_id = item.get("code")?.as_str()?.to_owned();
            let filename = item.get("filename")?.as_str()?;
            let path = resolve_external_path(root, filename);
            let source = std::fs::read_to_string(&path).ok();
            let location = item.get("location")?;
            let end_location = item.get("end_location")?;
            let start_line = location.get("row")?.as_u64()? as usize;
            let start_column = location.get("column")?.as_u64()? as usize;
            let end_line = end_location.get("row")?.as_u64()? as usize;
            let end_column = end_location.get("column")?.as_u64()? as usize;
            let (start_byte, end_byte) = source
                .as_deref()
                .map(|source| {
                    (
                        line_column_to_offset(source, start_line, start_column),
                        line_column_to_offset(source, end_line, end_column),
                    )
                })
                .unwrap_or((0, 0));
            let range = SourceRange::new(
                start_byte,
                end_byte.max(start_byte),
                start_line,
                start_column,
                end_line,
                end_column,
            )
            .ok()?;
            let severity = normalize_severity(
                item.get("severity")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("warning"),
            );
            let category = if rule_id.starts_with('E') || rule_id.starts_with('F') {
                DiagnosticCategory::Correctness
            } else if rule_id.starts_with('S') {
                DiagnosticCategory::Security
            } else {
                DiagnosticCategory::Style
            };
            let mut diagnostic = Diagnostic::new(
                "ruff",
                Language::Python,
                rule_id,
                severity,
                category,
                Confidence::High,
                path.clone(),
                range,
                item.get("message")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("Ruff diagnostic"),
            );
            if let Some(explanation) = item.get("url").and_then(serde_json::Value::as_str) {
                diagnostic = diagnostic.with_explanation(explanation);
            }
            if let Some(fix_value) = item.get("fix").filter(|value| !value.is_null()) {
                let edits = parse_ruff_fix(fix_value, source.as_deref());
                if !edits.is_empty() {
                    diagnostic = diagnostic.with_fix(codeforge_protocol::Fix::new(
                        fix_value
                            .get("message")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("Apply Ruff fix"),
                        edits,
                        false,
                    ));
                }
            }
            Some(diagnostic)
        })
        .collect()
}

fn parse_ruff_fix(value: &serde_json::Value, source: Option<&str>) -> Vec<TextEdit> {
    let Some(edits) = value.get("edits").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    edits
        .iter()
        .filter_map(|edit| {
            let content = edit.get("content")?.as_str()?.to_owned();
            let location = edit.get("location")?;
            let end_location = edit.get("end_location")?;
            let start_line = location.get("row")?.as_u64()? as usize;
            let start_column = location.get("column")?.as_u64()? as usize;
            let end_line = end_location.get("row")?.as_u64()? as usize;
            let end_column = end_location.get("column")?.as_u64()? as usize;
            let (start_byte, end_byte) = source
                .map(|source| {
                    (
                        line_column_to_offset(source, start_line, start_column),
                        line_column_to_offset(source, end_line, end_column),
                    )
                })
                .unwrap_or((0, 0));
            Some(TextEdit {
                range: SourceRange::new(
                    start_byte,
                    end_byte.max(start_byte),
                    start_line,
                    start_column,
                    end_line,
                    end_column,
                )
                .ok()?,
                replacement: content,
                description: None,
            })
        })
        .collect()
}

fn parse_clippy_json(output: &str, root: &Path, allowed: &HashSet<PathBuf>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for line in output.lines() {
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
        let Some(filename) = span.get("file_name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let path = resolve_external_path(root, filename);
        if !allowed.is_empty() && !allowed.contains(&path) {
            continue;
        }
        let start_line = span
            .get("line_start")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1) as usize;
        let start_column = span
            .get("column_start")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1) as usize;
        let end_line = span
            .get("line_end")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(start_line as u64) as usize;
        let end_column = span
            .get("column_end")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(start_column as u64) as usize;
        let start_byte = span
            .get("byte_start")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        let end_byte = span
            .get("byte_end")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(start_byte as u64) as usize;
        let Ok(range) = SourceRange::new(
            start_byte,
            end_byte.max(start_byte),
            start_line,
            start_column,
            end_line,
            end_column,
        ) else {
            continue;
        };
        let rule_id = message
            .get("code")
            .and_then(|code| code.get("code"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("clippy")
            .to_owned();
        diagnostics.push(Diagnostic::new(
            "clippy",
            Language::Rust,
            rule_id,
            normalize_severity(
                message
                    .get("level")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("warning"),
            ),
            DiagnosticCategory::Correctness,
            Confidence::High,
            path,
            range,
            message
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Clippy diagnostic"),
        ));
    }
    diagnostics
}

fn resolve_external_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn line_column_to_offset(source: &str, line: usize, column: usize) -> usize {
    let target_line = line.max(1) - 1;
    let target_column = column.max(1) - 1;
    let mut offset = 0usize;
    for (current_line, part) in source.split_inclusive('\n').enumerate() {
        if current_line == target_line {
            return offset
                + part
                    .chars()
                    .take(target_column)
                    .map(char::len_utf8)
                    .sum::<usize>();
        }
        offset += part.len();
    }
    source.len()
}

fn verify_prepared_syntax(preview: &PreparedChange) -> Result<VerificationResult, CoreError> {
    let started = Instant::now();
    for file in &preview.files {
        let language = file
            .relative_path
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(Language::from_extension)
            .ok_or_else(|| {
                CoreError::InvalidProject(format!(
                    "unsupported file type: {}",
                    file.relative_path.display()
                ))
            })?;
        let diagnostics = codeforge_engines::analyze_source(
            "preview-syntax",
            language,
            &file.relative_path,
            &file.after,
        );
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "PARSE-001")
        {
            return Ok(VerificationResult {
                syntax: VerificationCheck::failed(
                    "transformation produces invalid syntax",
                    started.elapsed().as_millis(),
                ),
                ..VerificationResult::default()
            });
        }
    }
    Ok(VerificationResult {
        syntax: VerificationCheck::passed(
            "Tree-sitter syntax check passed",
            started.elapsed().as_millis(),
        ),
        ..VerificationResult::default()
    })
}

fn verify_syntax_files(files: &[PathBuf]) -> VerificationCheck {
    let started = Instant::now();
    for file in files {
        let Some(language) = file
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(Language::from_extension)
        else {
            continue;
        };
        let source = match std::fs::read_to_string(file) {
            Ok(source) => source,
            Err(error) => {
                return VerificationCheck::failed(
                    format!("cannot read {}: {error}", file.display()),
                    started.elapsed().as_millis(),
                );
            }
        };
        let diagnostics =
            codeforge_engines::analyze_source("syntax-verifier", language, file, &source);
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "PARSE-001")
        {
            return VerificationCheck::failed(
                format!("syntax errors in {}", file.display()),
                started.elapsed().as_millis(),
            );
        }
    }
    VerificationCheck {
        status: VerificationStatus::Passed,
        message: Some(format!("{} source files parsed successfully", files.len())),
        duration_ms: Some(started.elapsed().as_millis()),
        command: None,
    }
}

fn read_package_json(root: &Path) -> Result<serde_json::Value, CoreError> {
    let bytes = std::fs::read(root.join("package.json"))?;
    serde_json::from_slice(&bytes).map_err(|error| CoreError::InvalidProject(error.to_string()))
}

fn has_script(package: &serde_json::Value, name: &str) -> bool {
    package
        .get("scripts")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|scripts| scripts.contains_key(name))
}

trait Tap: Sized {
    fn tap<T>(self, operation: impl FnOnce(&mut Self) -> T) -> Self {
        let mut value = self;
        let _ = operation(&mut value);
        value
    }
}

impl<T> Tap for T {}

fn git_branch_sync(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn unchanged_review_reuses_cached_diagnostics() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path().join("example.py"),
            "if value == None:\n    pass\n",
        )
        .expect("write");
        let engine = CodeForgeEngine::open(temp.path()).expect("engine");
        let first = engine
            .review(ReviewOptions::default())
            .await
            .expect("first");
        let second = engine
            .review(ReviewOptions::default())
            .await
            .expect("second");
        assert_eq!(first.diagnostics.len(), second.diagnostics.len());
        assert_eq!(
            first.diagnostics.first().map(|item| item.id.as_str()),
            second.diagnostics.first().map(|item| item.id.as_str())
        );
    }

    #[test]
    fn engine_opens_workspace_and_lists_tools() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("example.py"), "value = 1\n").expect("write");
        let engine = CodeForgeEngine::open(temp.path()).expect("engine");
        assert_eq!(
            engine.summary().name,
            temp.path().file_name().unwrap().to_string_lossy()
        );
        assert!(
            engine
                .engine_statuses()
                .iter()
                .any(|status| status.metadata.id == "python-builtin")
        );
    }
}
