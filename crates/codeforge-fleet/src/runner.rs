use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use codeforge_core::{CodeForgeEngine, ReviewOptions};
use codeforge_git::GitRepository;
use codeforge_protocol::{
    FileEdit, FleetRunSummary, Language, Patch, ProjectProfile, RepoRunStatus, RepoRunSummary,
    RiskLevel, SourceRange, TextEdit, Transformation, TransformationClass, VerificationResult,
    VerificationStatus,
};
use codeforge_transform::{ApplyOptions, PreparedChange, TransactionManager};
use sha2::Digest;
use tokio::sync::{Mutex, Semaphore};

use crate::FleetError;
use crate::cache::{CacheKey, FleetCache};
use crate::config::{FleetConfig, ResolvedRepository, is_generated};
use crate::detect::ProjectDetector;
use crate::evidence::{EvidenceInput, EvidenceSnapshot, EvidenceWriter};
use crate::pr::{PrManager, PrMode};
use crate::tools::{ToolContext, ToolOperation, ToolRegistry, ToolRunner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetCommand {
    Audit,
    Format,
    Review,
    Refactor,
    Optimize,
    Verify,
    Report,
}

impl FleetCommand {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Audit => "audit",
            Self::Format => "format",
            Self::Review => "review",
            Self::Refactor => "refactor",
            Self::Optimize => "optimize",
            Self::Verify => "verify",
            Self::Report => "report",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FleetRunOptions {
    pub command: FleetCommand,
    pub risk: RiskLevel,
    pub dry_run: bool,
    pub apply: bool,
    pub open_pr: bool,
    pub require_benchmark: bool,
    pub allow_protected: bool,
    pub allow_dirty: bool,
    pub include_external: bool,
    pub benchmark_samples: usize,
    pub report_dir: Option<PathBuf>,
}

impl FleetRunOptions {
    pub fn new(command: FleetCommand) -> Self {
        Self {
            command,
            risk: RiskLevel::Medium,
            dry_run: true,
            apply: false,
            open_pr: false,
            require_benchmark: false,
            allow_protected: false,
            allow_dirty: false,
            include_external: false,
            benchmark_samples: 5,
            report_dir: None,
        }
    }

    pub fn should_apply(&self) -> bool {
        self.apply || self.open_pr
    }
}

#[derive(Debug, Clone)]
pub struct RepoExecutionContext {
    pub run_id: String,
    pub report_dir: PathBuf,
    pub repository: ResolvedRepository,
    pub config: FleetConfig,
    pub options: FleetRunOptions,
    pub tools: Arc<ToolRegistry>,
}

#[async_trait]
pub trait RepoOperationExecutor: Send + Sync {
    async fn execute(&self, context: RepoExecutionContext) -> Result<RepoRunSummary, FleetError>;
}

#[derive(Debug, Clone)]
pub struct FleetRunner {
    config: FleetConfig,
}

impl FleetRunner {
    pub fn new(config: FleetConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &FleetConfig {
        &self.config
    }

    pub async fn run(&self, options: FleetRunOptions) -> Result<FleetRunSummary, FleetError> {
        let tools = Arc::new(ToolRegistry::with_all_builtins(
            &self.config.resolved_workspace_root,
        ));
        let executor = Arc::new(DefaultRepoExecutor::new(tools));
        self.run_with_executor(options, executor).await
    }

    pub async fn run_with_executor(
        &self,
        options: FleetRunOptions,
        executor: Arc<dyn RepoOperationExecutor>,
    ) -> Result<FleetRunSummary, FleetError> {
        let repositories = self.config.repositories()?;
        let run_id = uuid::Uuid::new_v4().simple().to_string();
        let report_dir = self
            .config
            .report_root(options.report_dir.as_deref())
            .join(&run_id);
        std::fs::create_dir_all(&report_dir)?;
        let tools = Arc::new(ToolRegistry::with_all_builtins(
            &self.config.resolved_workspace_root,
        ));
        let concurrency = self
            .config
            .fleet
            .concurrency
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|cpus| (cpus.get() / 2).max(1))
                    .unwrap_or(1)
            })
            .clamp(1, 4);
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let cache_root = report_dir.parent().unwrap_or(&report_dir).to_path_buf();
        let cache = Arc::new(Mutex::new(FleetCache::load(&cache_root)?));
        let config_hash = format!(
            "{:x}",
            sha2::Sha256::digest(toml::to_string(&self.config).unwrap_or_default().as_bytes())
        );
        let started_at = Utc::now();
        let mut tasks = tokio::task::JoinSet::new();
        for repository in repositories {
            let cache_key = if options.command == FleetCommand::Audit {
                let git = GitRepository::discover(&repository.path)?;
                if git.is_clean().await.unwrap_or(false) {
                    Some(CacheKey::new(
                        repository.name.clone(),
                        git.head_commit().await.ok(),
                        config_hash.clone(),
                        env!("CARGO_PKG_VERSION"),
                        "rules-v0.2",
                    ))
                } else {
                    None
                }
            } else {
                None
            };
            let context = RepoExecutionContext {
                run_id: run_id.clone(),
                report_dir: report_dir.clone(),
                repository,
                config: self.config.clone(),
                options: options.clone(),
                tools: tools.clone(),
            };
            let executor = executor.clone();
            let semaphore = semaphore.clone();
            let cache = cache.clone();
            tasks.spawn(async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|error| FleetError::Join(error.to_string()))?;
                if let Some(key) = &cache_key
                    && let Some(mut cached) = cache.lock().await.get(key)
                {
                    cached.message = format!("cache hit; {}", cached.message);
                    return Ok(cached);
                }
                let summary = executor.execute(context).await?;
                if let Some(key) = cache_key {
                    let mut cache = cache.lock().await;
                    cache.insert(key, summary.clone());
                    cache.save()?;
                }
                Ok(summary)
            });
        }

        let mut summaries = Vec::new();
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(summary)) => summaries.push(summary),
                Ok(Err(error)) => summaries.push(error_summary(error)),
                Err(error) => summaries.push(error_summary(FleetError::Join(error.to_string()))),
            }
        }
        summaries.sort_by(|left, right| left.repository.cmp(&right.repository));
        let summary = codeforge_protocol::FleetRunSummary::from_repositories(
            run_id,
            self.config.fleet.name.clone(),
            started_at,
            report_dir.clone(),
            summaries,
        );
        write_run_summary(&report_dir, &summary)?;
        Ok(summary)
    }
}

#[derive(Debug, Clone)]
pub struct DefaultRepoExecutor {
    _tools: Arc<ToolRegistry>,
}

impl DefaultRepoExecutor {
    pub fn new(tools: Arc<ToolRegistry>) -> Self {
        Self { _tools: tools }
    }
}

#[async_trait]
impl RepoOperationExecutor for DefaultRepoExecutor {
    async fn execute(&self, context: RepoExecutionContext) -> Result<RepoRunSummary, FleetError> {
        match context.options.command {
            FleetCommand::Audit | FleetCommand::Report => audit(context, false).await,
            FleetCommand::Review => audit(context, true).await,
            FleetCommand::Format => transform_format(context).await,
            FleetCommand::Refactor => transform_refactor(context).await,
            FleetCommand::Optimize => optimize(context).await,
            FleetCommand::Verify => verify(context).await,
        }
    }
}

async fn audit(
    context: RepoExecutionContext,
    include_external: bool,
) -> Result<RepoRunSummary, FleetError> {
    let engine = CodeForgeEngine::open(&context.repository.path)?;
    let detected = ProjectDetector::detect(&context.repository.path)?;
    let review = engine
        .review(ReviewOptions {
            languages: Vec::new(),
            changed_only: false,
            include_external: include_external || context.options.include_external,
        })
        .await?;
    let diagnostics = review.diagnostics;
    let branch = git_branch(&context.repository.path);
    let commit = git_commit(&context.repository.path);
    let verification = VerificationResult::default();
    let snapshot = EvidenceSnapshot {
        repository: context.repository.path.clone(),
        diagnostics: diagnostics.len(),
        verification: verification.clone(),
        files: Vec::new(),
        note: "read-only audit snapshot".to_owned(),
    };
    let written = EvidenceWriter::new(context.report_dir.clone()).write(EvidenceInput {
        id: format!("{}-{}", context.run_id, context.repository.name),
        repository: context.repository.path.clone(),
        commit,
        branch,
        risk: RiskLevel::Low,
        transformation_classes: Vec::new(),
        baseline: verification.clone(),
        after: verification.clone(),
        benchmark: None,
        patch: None,
        diagnostics: diagnostics.clone(),
        before_snapshot: snapshot.clone(),
        after_snapshot: snapshot,
    })?;
    Ok(RepoRunSummary {
        repository: context.repository.name,
        path: context.repository.path,
        status: if diagnostics.is_empty() {
            RepoRunStatus::Success
        } else {
            RepoRunStatus::Findings
        },
        languages: detected.profile.languages.clone(),
        project_profile: Some(detected.profile),
        findings: diagnostics.len(),
        pending_transformations: 0,
        risk: RiskLevel::Low,
        verification: Some(verification),
        evidence: Some(written.bundle),
        report_path: Some(written.report_markdown),
        message: if diagnostics.is_empty() {
            "audit completed without findings".to_owned()
        } else {
            format!("audit completed with {} findings", diagnostics.len())
        },
    })
}

async fn verify(context: RepoExecutionContext) -> Result<RepoRunSummary, FleetError> {
    let engine = CodeForgeEngine::open(&context.repository.path)?;
    let detected = ProjectDetector::detect(&context.repository.path)?;
    let verification = engine.verify(true).await?;
    let status = verification_status(&context.config, &verification);
    let snapshot = EvidenceSnapshot {
        repository: context.repository.path.clone(),
        diagnostics: 0,
        verification: verification.clone(),
        files: Vec::new(),
        note: "verification snapshot".to_owned(),
    };
    let written = write_evidence(
        &context,
        verification.clone(),
        VerificationResult::default(),
        verification.clone(),
        None,
        None,
        Vec::new(),
        snapshot.clone(),
        snapshot,
        Vec::new(),
    )?;
    Ok(summary_with_evidence(
        context.clone(),
        status,
        detected.profile,
        0,
        0,
        RiskLevel::Low,
        verification,
        written,
        format!("verification completed with {status:?}"),
    ))
}

async fn optimize(context: RepoExecutionContext) -> Result<RepoRunSummary, FleetError> {
    let engine = CodeForgeEngine::open(&context.repository.path)?;
    let detected = ProjectDetector::detect(&context.repository.path)?;
    let report = engine.optimize().await?;
    let benchmark = report.benchmark.clone();
    let has_benchmark = benchmark
        .as_ref()
        .is_some_and(|value| value.has_real_measurements());
    let status = if context.options.require_benchmark && !has_benchmark {
        RepoRunStatus::MissingTool
    } else if report.verification.accepted() || report.candidates.is_empty() {
        RepoRunStatus::Success
    } else {
        RepoRunStatus::VerificationFailure
    };
    let patch = report.patch.clone();
    let snapshot = EvidenceSnapshot {
        repository: context.repository.path.clone(),
        diagnostics: report.candidates.len(),
        verification: report.verification.clone(),
        files: patch
            .as_ref()
            .map(|value| value.files.iter().map(|file| file.file.clone()).collect())
            .unwrap_or_default(),
        note: report.message.clone(),
    };
    let written = write_evidence(
        &context,
        report.verification.clone(),
        VerificationResult::default(),
        report.verification.clone(),
        benchmark.clone(),
        patch,
        Vec::new(),
        snapshot.clone(),
        snapshot,
        vec![TransformationClass::PerformanceCandidate],
    )?;
    Ok(summary_with_evidence(
        context.clone(),
        status,
        detected.profile,
        report.candidates.len(),
        0,
        RiskLevel::High,
        report.verification,
        written,
        if has_benchmark {
            report.message
        } else if context.options.require_benchmark {
            "benchmark required but no measured comparison was available; no performance claim is made".to_owned()
        } else {
            "optimization proposal completed without a performance claim".to_owned()
        },
    ))
}

async fn transform_format(context: RepoExecutionContext) -> Result<RepoRunSummary, FleetError> {
    let engine = CodeForgeEngine::open(&context.repository.path)?;
    let detected = ProjectDetector::detect(&context.repository.path)?;
    let mut plans = Vec::new();
    for language in &detected.profile.languages {
        let files = workspace_files(engine.root(), *language);
        if files.is_empty() {
            continue;
        }
        plans.extend(context.tools.plans_for_language(
            *language,
            ToolOperation::FormatApply,
            &ToolContext {
                root: context.repository.path.clone(),
                files,
                check_only: !context.options.should_apply(),
            },
        ));
    }
    if plans.is_empty() {
        return Ok(RepoRunSummary {
            repository: context.repository.name,
            path: context.repository.path,
            status: RepoRunStatus::MissingTool,
            languages: detected.profile.languages.clone(),
            project_profile: Some(detected.profile),
            findings: 0,
            pending_transformations: 0,
            risk: RiskLevel::Low,
            verification: None,
            evidence: None,
            report_path: None,
            message: "no formatter adapter is available for this repository".to_owned(),
        });
    }
    let result = run_formatter_pipeline(context, detected.profile, plans).await?;
    Ok(result)
}

async fn run_formatter_pipeline(
    context: RepoExecutionContext,
    profile: ProjectProfile,
    plans: Vec<crate::tools::ToolPlan>,
) -> Result<RepoRunSummary, FleetError> {
    let sandbox = codeforge_core::Sandbox::create(&context.repository.path)?;
    let sandbox_root = sandbox.root().to_path_buf();
    let runner = ToolRunner::default();
    let mut tool_messages = Vec::new();
    for plan in plans {
        let mut sandbox_plan = plan;
        sandbox_plan.cwd = sandbox_root.clone();
        sandbox_plan.command = replace_paths(
            &sandbox_plan.command,
            &context.repository.path,
            &sandbox_root,
        );
        let execution = runner.execute(sandbox_plan).await?;
        if !execution.process.success() {
            tool_messages.push(format!(
                "{} exited {:?}: {}",
                execution.plan.adapter_id,
                execution.process.exit_code,
                execution.process.stderr.trim()
            ));
        }
    }
    let changed = changed_files(&context.repository.path, &sandbox_root)?;
    let transformations = transformations_for_changed_files(
        &context.repository.path,
        &sandbox_root,
        &changed,
        TransformationClass::StyleOnly,
    )?;
    let source_manager = TransactionManager::new(&context.repository.path)?;
    let source_preview = source_manager.preview(&transformations)?;
    if source_preview.files.is_empty() {
        return Ok(RepoRunSummary {
            repository: context.repository.name,
            path: context.repository.path,
            status: RepoRunStatus::Success,
            languages: profile.languages.clone(),
            project_profile: Some(profile),
            findings: 0,
            pending_transformations: 0,
            risk: RiskLevel::Low,
            verification: None,
            evidence: None,
            report_path: None,
            message: append_messages("formatter completed without changes", tool_messages),
        });
    }
    if !context.options.allow_protected
        && let Some(path) = source_preview
            .patch
            .files
            .iter()
            .map(|file| file.file.clone())
            .find(|path| context.config.policy.protected_paths.is_protected(path))
    {
        return Ok(policy_refusal(context, profile, path, "protected path"));
    }
    if source_preview.patch.files.len() > context.config.policy.max_changed_files
        || source_preview.patch.additions + source_preview.patch.deletions
            > context.config.policy.max_changed_lines
    {
        return Ok(policy_refusal(
            context,
            profile,
            PathBuf::from("diff-budget"),
            "SPLIT_REQUIRED",
        ));
    }
    if context.options.should_apply()
        && let Some((path, reason)) = apply_safety_violation(&context, &source_preview).await?
    {
        return Ok(policy_refusal(context, profile, path, &reason));
    }

    let baseline = engine_verification(&context.repository.path).await?;
    let sandbox_verification =
        verify_sandbox_with_transformations(&sandbox_root, &transformations, &baseline).await?;
    let status = if verification_passes(&context.config, &sandbox_verification) {
        RepoRunStatus::Findings
    } else {
        RepoRunStatus::VerificationFailure
    };
    let diagnostics =
        engine_diagnostics(&context.repository.path, context.options.include_external).await?;
    let snapshot = EvidenceSnapshot {
        repository: context.repository.path.clone(),
        diagnostics: diagnostics.len(),
        verification: baseline.clone(),
        files: source_preview
            .patch
            .files
            .iter()
            .map(|file| file.file.clone())
            .collect(),
        note: "formatter preview".to_owned(),
    };
    let written = write_evidence(
        &context,
        baseline.clone(),
        sandbox_verification.clone(),
        sandbox_verification.clone(),
        None,
        Some(source_preview.patch.clone()),
        diagnostics.clone(),
        snapshot.clone(),
        EvidenceSnapshot {
            verification: sandbox_verification.clone(),
            ..snapshot
        },
        vec![TransformationClass::StyleOnly],
    )?;
    let mut message = "formatter completed in an isolated workspace".to_owned();
    if context.options.should_apply() && verification_passes(&context.config, &sandbox_verification)
    {
        let branch = if context.options.open_pr {
            Some(PrManager::branch_name(
                &context.config.policy.branch_prefix,
                "format",
            ))
        } else {
            None
        };
        if let Some(branch) = &branch {
            PrManager
                .create_branch(
                    &context.repository.path,
                    branch,
                    context.config.policy.require_clean_tree,
                )
                .await?;
        }
        source_manager.apply(
            &source_preview,
            "Verified fleet formatter pass",
            sandbox_verification.clone(),
            Vec::new(),
            ApplyOptions {
                force: !context.config.policy.rollback_on_failure,
            },
        )?;
        if let Some(branch) = branch {
            let outcome = PrManager
                .publish(
                    &context.repository.path,
                    &branch,
                    &source_preview
                        .patch
                        .files
                        .iter()
                        .map(|file| file.file.clone())
                        .collect::<Vec<_>>(),
                    &PrMode::Open {
                        title: "chore(codeforge): safe refactoring pass".to_owned(),
                        body: format!(
                            "CodeForge v0.2 fleet formatter report: {}\n\nTransformation class: STYLE_ONLY\nVerification: {:?}\n",
                            written.report_markdown.display(),
                            sandbox_verification.evidence_level()
                        ),
                    },
                )
                .await?;
            message.push_str(&format!("; PR outcome: {outcome:?}"));
        }
        message.push_str("; changes applied transactionally");
    } else if status == RepoRunStatus::Findings {
        message.push_str("; source was not modified");
    }
    Ok(RepoRunSummary {
        repository: context.repository.name,
        path: context.repository.path,
        status,
        languages: profile.languages.clone(),
        project_profile: Some(profile),
        findings: diagnostics.len(),
        pending_transformations: source_preview.patch.files.len(),
        risk: RiskLevel::Low,
        verification: Some(sandbox_verification),
        evidence: Some(written.bundle),
        report_path: Some(written.report_markdown),
        message: append_messages(&message, tool_messages),
    })
}

async fn transform_refactor(context: RepoExecutionContext) -> Result<RepoRunSummary, FleetError> {
    let detected = ProjectDetector::detect(&context.repository.path)?;
    let diagnostics =
        engine_diagnostics(&context.repository.path, context.options.include_external).await?;
    let transformations = transformations_from_diagnostics(&diagnostics, context.options.risk);
    if transformations.is_empty() {
        return Ok(RepoRunSummary {
            repository: context.repository.name,
            path: context.repository.path,
            status: RepoRunStatus::Success,
            languages: detected.profile.languages.clone(),
            project_profile: Some(detected.profile),
            findings: diagnostics.len(),
            pending_transformations: 0,
            risk: context.options.risk,
            verification: None,
            evidence: None,
            report_path: None,
            message: "no eligible transformations for the requested risk".to_owned(),
        });
    }
    let source_manager = TransactionManager::new(&context.repository.path)?;
    let source_preview = source_manager.preview(&transformations)?;
    if !context.options.allow_protected
        && let Some(path) = source_preview
            .patch
            .files
            .iter()
            .map(|file| file.file.clone())
            .find(|path| context.config.policy.protected_paths.is_protected(path))
    {
        return Ok(policy_refusal(
            context,
            detected.profile,
            path,
            "protected path",
        ));
    }
    if source_preview.patch.files.len() > context.config.policy.max_changed_files
        || source_preview.patch.additions + source_preview.patch.deletions
            > context.config.policy.max_changed_lines
    {
        return Ok(policy_refusal(
            context,
            detected.profile,
            PathBuf::from("diff-budget"),
            "SPLIT_REQUIRED",
        ));
    }
    if context.options.should_apply()
        && let Some((path, reason)) = apply_safety_violation(&context, &source_preview).await?
    {
        return Ok(policy_refusal(context, detected.profile, path, &reason));
    }
    let classes = transformations
        .iter()
        .map(|transformation| transformation.transformation_class)
        .collect::<Vec<_>>();
    let baseline = engine_verification(&context.repository.path).await?;
    let sandbox = codeforge_core::Sandbox::create(&context.repository.path)?;
    let sandbox_manager = TransactionManager::new(sandbox.root())?;
    let sandbox_preview = sandbox_manager.preview(&transformations)?;
    sandbox_manager.apply(
        &sandbox_preview,
        "fleet refactor sandbox",
        VerificationResult::default(),
        Vec::new(),
        ApplyOptions { force: true },
    )?;
    let sandbox_verification = CodeForgeEngine::open(sandbox.root())?.verify(true).await?;
    let status = if verification_passes(&context.config, &sandbox_verification) {
        RepoRunStatus::Findings
    } else {
        RepoRunStatus::VerificationFailure
    };
    let snapshot = EvidenceSnapshot {
        repository: context.repository.path.clone(),
        diagnostics: diagnostics.len(),
        verification: baseline.clone(),
        files: source_preview
            .patch
            .files
            .iter()
            .map(|file| file.file.clone())
            .collect(),
        note: "refactor preview".to_owned(),
    };
    let written = write_evidence(
        &context,
        baseline.clone(),
        sandbox_verification.clone(),
        sandbox_verification.clone(),
        None,
        Some(source_preview.patch.clone()),
        diagnostics.clone(),
        snapshot.clone(),
        EvidenceSnapshot {
            verification: sandbox_verification.clone(),
            ..snapshot
        },
        classes.clone(),
    )?;
    let mut message = "refactor transformations were previewed in isolation".to_owned();
    if context.options.should_apply() && verification_passes(&context.config, &sandbox_verification)
    {
        let branch = context
            .options
            .open_pr
            .then(|| PrManager::branch_name(&context.config.policy.branch_prefix, "refactor"));
        if let Some(branch) = &branch {
            PrManager
                .create_branch(
                    &context.repository.path,
                    branch,
                    context.config.policy.require_clean_tree,
                )
                .await?;
        }
        source_manager.apply(
            &source_preview,
            "Verified fleet refactoring pass",
            sandbox_verification.clone(),
            Vec::new(),
            ApplyOptions {
                force: !context.config.policy.rollback_on_failure,
            },
        )?;
        if let Some(branch) = branch {
            let outcome = PrManager
                .publish(
                    &context.repository.path,
                    &branch,
                    &source_preview
                        .patch
                        .files
                        .iter()
                        .map(|file| file.file.clone())
                        .collect::<Vec<_>>(),
                    &PrMode::Open {
                        title: "refactor: verified structural cleanup".to_owned(),
                        body: format!(
                            "CodeForge v0.2 fleet refactor report: {}\n\nClasses: {}\nVerification: {:?}\n",
                            written.report_markdown.display(),
                            classes
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", "),
                            sandbox_verification.evidence_level()
                        ),
                    },
                )
                .await?;
            message.push_str(&format!("; PR outcome: {outcome:?}"));
        }
        message.push_str("; changes applied transactionally");
    } else {
        message.push_str("; source was not modified");
    }
    Ok(RepoRunSummary {
        repository: context.repository.name,
        path: context.repository.path,
        status,
        languages: detected.profile.languages.clone(),
        project_profile: Some(detected.profile),
        findings: diagnostics.len(),
        pending_transformations: source_preview.patch.files.len(),
        risk: classes
            .iter()
            .map(|class| class.default_risk())
            .max()
            .unwrap_or(context.options.risk),
        verification: Some(sandbox_verification),
        evidence: Some(written.bundle),
        report_path: Some(written.report_markdown),
        message,
    })
}

#[allow(clippy::too_many_arguments)]
fn summary_with_evidence(
    context: RepoExecutionContext,
    status: RepoRunStatus,
    profile: ProjectProfile,
    findings: usize,
    pending: usize,
    risk: RiskLevel,
    verification: VerificationResult,
    written: crate::evidence::WrittenEvidence,
    message: String,
) -> RepoRunSummary {
    RepoRunSummary {
        repository: context.repository.name,
        path: context.repository.path,
        status,
        languages: profile.languages.clone(),
        project_profile: Some(profile),
        findings,
        pending_transformations: pending,
        risk,
        verification: Some(verification),
        evidence: Some(written.bundle),
        report_path: Some(written.report_markdown),
        message,
    }
}

fn policy_refusal(
    context: RepoExecutionContext,
    profile: ProjectProfile,
    path: PathBuf,
    reason: &str,
) -> RepoRunSummary {
    RepoRunSummary {
        repository: context.repository.name,
        path: context.repository.path,
        status: RepoRunStatus::SafetyRefusal,
        languages: profile.languages.clone(),
        project_profile: Some(profile),
        findings: 0,
        pending_transformations: 0,
        risk: RiskLevel::Low,
        verification: None,
        evidence: None,
        report_path: None,
        message: format!("{reason}: {}", path.display()),
    }
}

async fn engine_verification(root: &Path) -> Result<VerificationResult, FleetError> {
    Ok(CodeForgeEngine::open(root)?.verify(false).await?)
}

async fn engine_diagnostics(
    root: &Path,
    include_external: bool,
) -> Result<Vec<codeforge_protocol::Diagnostic>, FleetError> {
    Ok(CodeForgeEngine::open(root)?
        .review(ReviewOptions {
            languages: Vec::new(),
            changed_only: false,
            include_external,
        })
        .await?
        .diagnostics)
}

async fn verify_sandbox_with_transformations(
    sandbox_root: &Path,
    transformations: &[Transformation],
    baseline: &VerificationResult,
) -> Result<VerificationResult, FleetError> {
    let manager = TransactionManager::new(sandbox_root)?;
    let preview = manager.preview(transformations)?;
    if !preview.files.is_empty() {
        manager.apply(
            &preview,
            "fleet formatter sandbox",
            baseline.clone(),
            Vec::new(),
            ApplyOptions { force: true },
        )?;
    }
    Ok(CodeForgeEngine::open(sandbox_root)?.verify(true).await?)
}

fn verification_passes(config: &FleetConfig, result: &VerificationResult) -> bool {
    if result.syntax.status == VerificationStatus::Failed
        || result.equivalence.status == VerificationStatus::Failed
        || result.typecheck.status == VerificationStatus::Failed
    {
        return false;
    }
    if config.policy.require_build && result.build.status == VerificationStatus::Failed {
        return false;
    }
    if config.policy.require_tests && result.tests.status == VerificationStatus::Failed {
        return false;
    }
    true
}

fn verification_status(config: &FleetConfig, result: &VerificationResult) -> RepoRunStatus {
    if [
        &result.syntax,
        &result.typecheck,
        &result.build,
        &result.tests,
        &result.equivalence,
    ]
    .iter()
    .any(|check| check.status == VerificationStatus::Failed)
    {
        RepoRunStatus::VerificationFailure
    } else if config.policy.require_tests && result.tests.status != VerificationStatus::Passed
        || config.policy.require_build && result.build.status != VerificationStatus::Passed
    {
        RepoRunStatus::MissingTool
    } else {
        RepoRunStatus::Success
    }
}

fn transformations_from_diagnostics(
    diagnostics: &[codeforge_protocol::Diagnostic],
    risk: RiskLevel,
) -> Vec<Transformation> {
    diagnostics
        .iter()
        .flat_map(|diagnostic| {
            diagnostic
                .fixes
                .iter()
                .filter(|fix| fix.safe)
                .map(move |fix| {
                    let class = diagnostic
                        .transformation_class
                        .unwrap_or_else(|| infer_class(diagnostic.category));
                    (diagnostic, fix, class)
                })
        })
        .filter(|(_, _, class)| {
            class.is_permitted_by(risk) && class.default_risk() != RiskLevel::VeryHigh
        })
        .map(|(diagnostic, fix, class)| {
            Transformation::new(
                diagnostic.engine.clone(),
                diagnostic.language,
                fix.title.clone(),
                diagnostic.message.clone(),
                vec![FileEdit {
                    file: diagnostic.file.clone(),
                    edits: fix.edits.clone(),
                }],
            )
            .with_class(class)
        })
        .collect()
}

fn infer_class(category: codeforge_protocol::DiagnosticCategory) -> TransformationClass {
    match category {
        codeforge_protocol::DiagnosticCategory::Style => TransformationClass::StyleOnly,
        codeforge_protocol::DiagnosticCategory::DeadCode => TransformationClass::DeadCode,
        codeforge_protocol::DiagnosticCategory::Complexity => {
            TransformationClass::ComplexityReduction
        }
        codeforge_protocol::DiagnosticCategory::Performance => {
            TransformationClass::PerformanceCandidate
        }
        codeforge_protocol::DiagnosticCategory::ApiMisuse => TransformationClass::ApiRefactor,
        _ => TransformationClass::SafeAstFix,
    }
}

fn changed_files(source_root: &Path, sandbox_root: &Path) -> Result<Vec<PathBuf>, FleetError> {
    let mut changed = Vec::new();
    for entry in walkdir::WalkDir::new(sandbox_root)
        .follow_links(false)
        .into_iter()
    {
        let entry = entry.map_err(|error| FleetError::Io(std::io::Error::other(error)))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(sandbox_root)
            .unwrap_or(entry.path());
        let source = source_root.join(relative);
        if !source.is_file() {
            continue;
        }
        let sandbox_content = std::fs::read(entry.path())?;
        let source_content = std::fs::read(&source)?;
        let source_text = String::from_utf8_lossy(&source_content);
        if sandbox_content != source_content && !is_generated(relative, &source_text) {
            changed.push(relative.to_path_buf());
        }
    }
    Ok(changed)
}

async fn apply_safety_violation(
    context: &RepoExecutionContext,
    preview: &PreparedChange,
) -> Result<Option<(PathBuf, String)>, FleetError> {
    if context.config.policy.require_clean_tree {
        let repository = GitRepository::discover(&context.repository.path)?;
        if !repository.is_clean().await? {
            return Ok(Some((
                PathBuf::from("working-tree"),
                "clean working tree is required before apply; --allow-dirty is review/report only"
                    .to_owned(),
            )));
        }
    }
    for file in &preview.patch.files {
        if !context.options.allow_protected
            && context
                .config
                .policy
                .protected_paths
                .is_protected(&file.file)
        {
            return Ok(Some((
                file.file.clone(),
                "protected path requires --allow-protected".to_owned(),
            )));
        }
        let path = context.repository.path.join(&file.file);
        let content = std::fs::read_to_string(path)?;
        if is_generated(&file.file, &content) {
            return Ok(Some((
                file.file.clone(),
                "generated code is excluded by default".to_owned(),
            )));
        }
    }
    Ok(None)
}

fn transformations_for_changed_files(
    source_root: &Path,
    sandbox_root: &Path,
    files: &[PathBuf],
    class: TransformationClass,
) -> Result<Vec<Transformation>, FleetError> {
    let mut transformations = Vec::new();
    for relative in files {
        let source = source_root.join(relative);
        let after = sandbox_root.join(relative);
        let before_text = std::fs::read_to_string(&source)?;
        let after_text = std::fs::read_to_string(&after)?;
        let language = relative
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(Language::from_extension)
            .unwrap_or(Language::Markdown);
        let end_line = before_text.lines().count().max(1);
        transformations.push(
            Transformation::new(
                "fleet-format",
                language,
                "Formatter output",
                "Apply verified formatter output from an isolated workspace",
                vec![FileEdit {
                    file: relative.clone(),
                    edits: vec![TextEdit {
                        range: SourceRange::new(0, before_text.len(), 1, 1, end_line, 1)?,
                        replacement: after_text,
                        description: Some("formatter output".to_owned()),
                    }],
                }],
            )
            .with_class(class),
        );
    }
    Ok(transformations)
}

fn workspace_files(root: &Path, language: Language) -> Vec<PathBuf> {
    CodeForgeEngine::open(root).map_or_else(
        |_| Vec::new(),
        |engine| {
            if !engine.summary().languages.contains_key(&language) {
                return Vec::new();
            }
            walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .and_then(Language::from_extension)
                        == Some(language)
                })
                .map(|entry| {
                    entry
                        .path()
                        .strip_prefix(root)
                        .unwrap_or(entry.path())
                        .to_path_buf()
                })
                .collect()
        },
    )
}

fn replace_paths(command: &[String], source_root: &Path, sandbox_root: &Path) -> Vec<String> {
    command
        .iter()
        .map(|argument| {
            let path = Path::new(argument);
            if path.is_absolute() && path.starts_with(source_root) {
                sandbox_root
                    .join(path.strip_prefix(source_root).unwrap_or(path))
                    .to_string_lossy()
                    .into_owned()
            } else {
                argument.clone()
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn write_evidence(
    context: &RepoExecutionContext,
    baseline: VerificationResult,
    _after: VerificationResult,
    evidence_after: VerificationResult,
    benchmark: Option<codeforge_protocol::BenchmarkResult>,
    patch: Option<Patch>,
    diagnostics: Vec<codeforge_protocol::Diagnostic>,
    before_snapshot: EvidenceSnapshot,
    after_snapshot: EvidenceSnapshot,
    classes: Vec<TransformationClass>,
) -> Result<crate::evidence::WrittenEvidence, FleetError> {
    EvidenceWriter::new(context.report_dir.clone()).write(EvidenceInput {
        id: format!("{}-{}", context.run_id, context.repository.name),
        repository: context.repository.path.clone(),
        commit: git_commit(&context.repository.path),
        branch: git_branch(&context.repository.path),
        risk: classes
            .iter()
            .map(|class| class.default_risk())
            .max()
            .unwrap_or(RiskLevel::Low),
        transformation_classes: classes,
        baseline,
        after: evidence_after,
        benchmark,
        patch,
        diagnostics,
        before_snapshot,
        after_snapshot,
    })
}

fn append_messages(base: &str, messages: Vec<String>) -> String {
    if messages.is_empty() {
        base.to_owned()
    } else {
        format!("{base}; tools: {}", messages.join(" | "))
    }
}

fn git_branch(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn git_commit(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn write_run_summary(
    root: &Path,
    summary: &codeforge_protocol::FleetRunSummary,
) -> Result<(), FleetError> {
    std::fs::create_dir_all(root)?;
    let json_path = root.join("run.json");
    let markdown_path = root.join("summary.md");
    std::fs::write(&json_path, serde_json::to_vec_pretty(summary)?)?;
    let mut markdown = format!(
        "# CodeForge Fleet Run\n\nFleet: {}\n\nRun: {}\n\nStatus: {:?}\n\n",
        summary.fleet_name, summary.run_id, summary.status
    );
    for repository in &summary.repositories {
        markdown.push_str(&format!(
            "## {}\n\nStatus: {:?}\n\nLanguages: {}\n\nFindings: {}\n\nPending transformations: {}\n\nRisk: {:?}\n\n{}\n\n",
            repository.repository,
            repository.status,
            repository
                .languages
                .iter()
                .map(|language| language.display_name())
                .collect::<Vec<_>>()
                .join(", "),
            repository.findings,
            repository.pending_transformations,
            repository.risk,
            repository.message,
        ));
    }
    std::fs::write(markdown_path, markdown)?;
    Ok(())
}

fn error_summary(error: FleetError) -> RepoRunSummary {
    let message = error.to_string();
    RepoRunSummary {
        repository: "unknown".to_owned(),
        path: PathBuf::from("."),
        status: RepoRunStatus::TransformationFailure,
        languages: Vec::new(),
        project_profile: None,
        findings: 0,
        pending_transformations: 0,
        risk: RiskLevel::High,
        verification: None,
        evidence: None,
        report_path: None,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeforge_protocol::{FleetRunStatus, RepoRunStatus};

    #[tokio::test]
    async fn three_repo_fixture_returns_partial_success() {
        let temp = tempfile::tempdir().expect("tempdir");
        for name in ["success", "verification-failure", "missing-tool"] {
            std::fs::create_dir_all(temp.path().join(name)).expect("repo");
        }
        let config_path = temp.path().join("fleet.toml");
        std::fs::write(
            &config_path,
            r#"
[fleet]
name = "fixture"
workspace_root = "."
repositories = ["success", "verification-failure", "missing-tool"]
"#,
        )
        .expect("config");
        let config = FleetConfig::load(&config_path).expect("fleet config");
        let executor = Arc::new(FixtureExecutor);
        let runner = FleetRunner::new(config);
        let summary = runner
            .run_with_executor(FleetRunOptions::new(FleetCommand::Verify), executor)
            .await
            .expect("fleet run");
        assert_eq!(summary.status, FleetRunStatus::PartialSuccess);
        assert!(
            summary
                .repositories
                .iter()
                .any(|repo| repo.status == RepoRunStatus::Success)
        );
        assert!(
            summary
                .repositories
                .iter()
                .any(|repo| repo.status == RepoRunStatus::VerificationFailure)
        );
        assert!(
            summary
                .repositories
                .iter()
                .any(|repo| repo.status == RepoRunStatus::MissingTool)
        );
    }

    #[derive(Debug)]
    struct FixtureExecutor;

    #[async_trait]
    impl RepoOperationExecutor for FixtureExecutor {
        async fn execute(
            &self,
            context: RepoExecutionContext,
        ) -> Result<RepoRunSummary, FleetError> {
            let status = match context.repository.name.as_str() {
                "success" => RepoRunStatus::Success,
                "verification-failure" => RepoRunStatus::VerificationFailure,
                _ => RepoRunStatus::MissingTool,
            };
            Ok(RepoRunSummary {
                repository: context.repository.name,
                path: context.repository.path,
                status,
                languages: vec![Language::Rust],
                project_profile: None,
                findings: 0,
                pending_transformations: 0,
                risk: RiskLevel::Low,
                verification: None,
                evidence: None,
                report_path: None,
                message: "fixture".to_owned(),
            })
        }
    }
}
