use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use codeforge_core::{CodeForgeEngine, ReviewOptions};
use codeforge_fleet::{FleetCommand as FleetOperation, FleetConfig, FleetRunOptions, FleetRunner};
use codeforge_protocol::{Language, Severity, diagnostics_to_sarif};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    name = "codeforge",
    version,
    about = "Local-first, verification-driven code transformation runtime"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Detect project structure, local engines, and quick diagnostics.
    Scan(CommonArgs),
    /// Run the review pipeline and print diagnostics.
    Lint(CommonArgs),
    /// Run the full review pipeline.
    Review(CommonArgs),
    /// Format safe style-only changes through the local formatter adapters.
    Beautify(CommonArgs),
    /// Show safe and unsafe transformations proposed by built-in adapters.
    Refactor(CommonArgs),
    /// Generate a performance candidate, verify it in a sandbox, and benchmark it.
    Optimize(CommonArgs),
    /// Run syntax checks plus configured/local build and test verification.
    Verify(CommonArgs),
    /// Run a configured or auto-detected benchmark command.
    Benchmark(BenchmarkArgs),
    /// Run read-only review and verification for CI.
    Ci(CiArgs),
    /// Orchestrate independent repository transactions across a fleet.
    Fleet(FleetArgs),
}

#[derive(Debug, Args, Clone)]
struct CommonArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
    /// Emit SARIF 2.1.0. Valid for review-oriented commands.
    #[arg(long)]
    sarif: bool,
    /// Analyze only files reported by Git.
    #[arg(long)]
    changed: bool,
    /// Restrict analysis to a language. May be repeated.
    #[arg(long = "language")]
    languages: Vec<String>,
    /// Restrict output to diagnostics from an engine whose id contains this value.
    #[arg(long = "engine")]
    engines: Vec<String>,
    /// Include available external lint engines.
    #[arg(long)]
    external: bool,
}

#[derive(Debug, Args)]
struct BenchmarkArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Measured samples. Warm-up runs are always one.
    #[arg(long, default_value_t = 5)]
    samples: usize,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct CiArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Analyze only files reported by Git.
    #[arg(long)]
    changed: bool,
    /// Restrict output to diagnostics at or above this risk threshold.
    #[arg(long, default_value = "low")]
    risk: String,
    #[arg(long)]
    sarif: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args, Clone)]
struct FleetArgs {
    #[command(subcommand)]
    command: FleetSubcommand,
}

#[derive(Debug, Subcommand, Clone)]
enum FleetSubcommand {
    Audit(FleetRunArgs),
    Format(FleetRunArgs),
    Review(FleetRunArgs),
    Refactor(FleetRunArgs),
    Optimize(FleetRunArgs),
    Verify(FleetRunArgs),
    Report(FleetRunArgs),
}

#[derive(Debug, Args, Clone)]
struct FleetRunArgs {
    #[arg(long, default_value = "fleet.toml")]
    config: PathBuf,
    #[arg(long, default_value = "medium")]
    risk: String,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    apply: bool,
    #[arg(long)]
    open_pr: bool,
    #[arg(long)]
    require_benchmark: bool,
    #[arg(long)]
    allow_protected: bool,
    #[arg(long)]
    allow_dirty: bool,
    #[arg(long)]
    external: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    sarif: bool,
    #[arg(long)]
    report_dir: Option<PathBuf>,
    #[arg(long, default_value_t = 5)]
    samples: usize,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let result = match cli.command {
        Command::Scan(args) => scan(args).await,
        Command::Lint(args) | Command::Review(args) => review(args).await,
        Command::Beautify(args) => beautify(args).await,
        Command::Refactor(args) => refactor(args).await,
        Command::Optimize(args) => optimize(args).await,
        Command::Verify(args) => verify(args).await,
        Command::Benchmark(args) => benchmark(args).await,
        Command::Ci(args) => ci(args).await,
        Command::Fleet(args) => fleet(args).await,
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(exit_code_for(&error))
        }
    }
}

fn exit_code_for(error: &anyhow::Error) -> u8 {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("configuration") || message.contains("fleet.toml") {
        3
    } else if message.contains("verification") {
        2
    } else if message.contains("missing tool") || message.contains("not found") {
        5
    } else if message.contains("safety") || message.contains("protected") {
        6
    } else if message.contains("transform") {
        4
    } else {
        1
    }
}

async fn scan(args: CommonArgs) -> Result<()> {
    let engine = open(&args.path)?;
    if args.json {
        print_json(&serde_json::json!({
            "workspace": engine.summary(),
            "engines": engine.engine_statuses(),
        }))?;
        return Ok(());
    }
    let summary = engine.summary();
    println!("Workspace: {}", summary.root.display());
    println!("Project:   {}", summary.name);
    println!(
        "Branch:    {}",
        summary
            .git_branch
            .as_deref()
            .unwrap_or("not a Git worktree")
    );
    println!(
        "Markers:   {}",
        if summary.project_markers.is_empty() {
            "none".to_owned()
        } else {
            summary.project_markers.join(", ")
        }
    );
    println!("Languages:");
    for (language, stats) in &summary.languages {
        println!(
            "  {:<12} {:>5} files  {:>10} bytes",
            language.display_name(),
            stats.files,
            stats.bytes
        );
    }
    println!("Engines:");
    for status in engine.engine_statuses() {
        println!(
            "  {:<20} {}",
            status.metadata.name,
            if status.available {
                "available"
            } else {
                "missing"
            }
        );
        if let Some(reason) = status.reason {
            println!("    {reason}");
        }
    }
    Ok(())
}

async fn review(args: CommonArgs) -> Result<()> {
    let engine = open(&args.path)?;
    let report = engine.review(review_options(&args)).await?;
    let diagnostics = filter_diagnostics(report.diagnostics, &args.engines);
    if args.sarif {
        print_json(&diagnostics_to_sarif(&diagnostics))?;
    } else if args.json {
        print_json(&serde_json::json!({
            "summary": engine.summary(),
            "files_analyzed": report.files_analyzed,
            "languages": report.languages,
            "engines_used": report.engines_used,
            "duration_ms": report.duration_ms,
            "diagnostics": diagnostics,
        }))?;
    } else {
        println!(
            "Analyzed {} files in {} ms: {} diagnostics",
            report.files_analyzed,
            report.duration_ms,
            diagnostics.len()
        );
        for diagnostic in diagnostics {
            println!(
                "{}:{}:{} [{}] {} ({}, {})",
                diagnostic.file.display(),
                diagnostic.range.start_line,
                diagnostic.range.start_column,
                diagnostic.severity_label(),
                diagnostic.message,
                diagnostic.rule_id,
                diagnostic.engine
            );
            if !diagnostic.fixes.is_empty() {
                println!(
                    "  fixes: {}",
                    diagnostic
                        .fixes
                        .iter()
                        .map(|fix| fix.title.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }
    Ok(())
}

async fn refactor(args: CommonArgs) -> Result<()> {
    let engine = open(&args.path)?;
    let report = engine.review(review_options(&args)).await?;
    let diagnostics = filter_diagnostics(report.diagnostics, &args.engines)
        .into_iter()
        .filter(|diagnostic| !diagnostic.fixes.is_empty())
        .collect::<Vec<_>>();
    if args.json {
        print_json(&diagnostics)?;
    } else {
        println!(
            "{} transformations with previewable edits",
            diagnostics.len()
        );
        for diagnostic in diagnostics {
            println!(
                "{}:{} [{}] {}",
                diagnostic.file.display(),
                diagnostic.range.start_line,
                diagnostic.rule_id,
                diagnostic.message
            );
            for fix in diagnostic.fixes {
                println!(
                    "  {} ({})",
                    fix.title,
                    if fix.safe { "safe" } else { "review required" }
                );
            }
        }
    }
    Ok(())
}

async fn optimize(args: CommonArgs) -> Result<()> {
    let engine = open(&args.path)?;
    let report = engine.optimize().await?;
    if args.json {
        print_json(&report)?;
    } else {
        println!("{}", report.message);
        for candidate in &report.candidates {
            println!(
                "  {}:{} {} [{}]",
                candidate.file.display(),
                candidate.line,
                candidate.title,
                candidate.rule_id
            );
        }
        println!("Evidence: {:?}", report.verification.evidence_level());
        if let Some(benchmark) = report.benchmark {
            if benchmark.has_real_measurements() {
                println!(
                    "Benchmark {}: {:.3} ms -> {:.3} ms ({:+.2}%)",
                    benchmark.metric,
                    benchmark.before.median,
                    benchmark.after.median,
                    benchmark.delta_percent
                );
            }
        } else {
            println!("Benchmark: no measured comparison available");
        }
    }
    Ok(())
}

async fn verify(args: CommonArgs) -> Result<()> {
    let engine = open(&args.path)?;
    let result = engine.verify(true).await?;
    if args.json {
        print_json(&result)?;
    } else {
        println!("Syntax:      {:?}", result.syntax.status);
        println!("Typecheck:   {:?}", result.typecheck.status);
        println!("Build:       {:?}", result.build.status);
        println!("Tests:       {:?}", result.tests.status);
        println!("Evidence:    {:?}", result.evidence_level());
    }
    Ok(())
}

async fn benchmark(args: BenchmarkArgs) -> Result<()> {
    let engine = open(&args.path)?;
    let snapshot = engine.benchmark(args.samples).await?;
    if args.json {
        print_json(&snapshot)?;
    } else {
        println!("Command: {}", snapshot.command.join(" "));
        println!(
            "Samples: {}  median: {:.3} {}  variance: {:.6}",
            snapshot.summary.samples.len(),
            snapshot.summary.median,
            snapshot.summary.unit,
            snapshot.summary.variance
        );
    }
    Ok(())
}

async fn beautify(args: CommonArgs) -> Result<()> {
    let root = args
        .path
        .canonicalize()
        .with_context(|| format!("cannot open {}", args.path.display()))?;
    let repository = root
        .file_name()
        .and_then(|name| name.to_str())
        .context("workspace path has no repository name")?
        .to_owned();
    let workspace = root.parent().context("workspace path has no parent")?;
    let config_path = std::env::temp_dir().join(format!(
        "codeforge-beautify-{}.toml",
        uuid::Uuid::new_v4().simple()
    ));
    let config_text = format!(
        "[fleet]\nname = \"single-repository\"\nworkspace_root = {:?}\nrepositories = [{:?}]\n",
        workspace.to_string_lossy().replace('\\', "/"),
        repository,
    );
    let config = FleetConfig::from_toml_str(&config_text, &config_path)?;
    let mut options = FleetRunOptions::new(FleetOperation::Format);
    options.dry_run = true;
    options.include_external = args.external;
    let summary = FleetRunner::new(config).run(options).await?;
    print_fleet_summary(&summary, false, false)?;
    fleet_exit(&summary)
}

async fn ci(args: CiArgs) -> Result<()> {
    let _risk = parse_risk(&args.risk)?;
    let engine = open(&args.path)?;
    let mut report = engine
        .review(ReviewOptions {
            languages: Vec::new(),
            changed_only: args.changed,
            include_external: true,
        })
        .await?;
    let verification = engine.verify(true).await?;
    if let Some(path) = &args.sarif {
        let sarif = diagnostics_to_sarif(&report.diagnostics);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&sarif)?)?;
    }
    if args.json {
        print_json(&serde_json::json!({
            "diagnostics": report.diagnostics,
            "verification": verification,
        }))?;
    } else {
        println!("CI diagnostics: {}", report.diagnostics.len());
        println!("Syntax:        {:?}", verification.syntax.status);
        println!("Build:         {:?}", verification.build.status);
        println!("Tests:         {:?}", verification.tests.status);
    }
    if matches!(
        verification.syntax.status,
        codeforge_protocol::VerificationStatus::Failed
    ) || matches!(
        verification.build.status,
        codeforge_protocol::VerificationStatus::Failed
    ) || matches!(
        verification.tests.status,
        codeforge_protocol::VerificationStatus::Failed
    ) {
        anyhow::bail!("verification failure");
    }
    if !report.diagnostics.is_empty() {
        report.diagnostics.clear();
        anyhow::bail!("findings");
    }
    Ok(())
}

async fn fleet(args: FleetArgs) -> Result<()> {
    let (operation, run_args) = match &args.command {
        FleetSubcommand::Audit(args) => (FleetOperation::Audit, args),
        FleetSubcommand::Format(args) => (FleetOperation::Format, args),
        FleetSubcommand::Review(args) => (FleetOperation::Review, args),
        FleetSubcommand::Refactor(args) => (FleetOperation::Refactor, args),
        FleetSubcommand::Optimize(args) => (FleetOperation::Optimize, args),
        FleetSubcommand::Verify(args) => (FleetOperation::Verify, args),
        FleetSubcommand::Report(args) => (FleetOperation::Report, args),
    };
    if run_args.dry_run && run_args.apply {
        anyhow::bail!("conflicting flags: --dry-run and --apply");
    }
    let config = FleetConfig::load(&run_args.config)
        .with_context(|| format!("configuration failure: {}", run_args.config.display()))?;
    let mut options = FleetRunOptions::new(operation);
    options.risk = parse_risk(&run_args.risk)?;
    options.dry_run = run_args.dry_run || !(run_args.apply || run_args.open_pr);
    options.apply = run_args.apply || run_args.open_pr;
    options.open_pr = run_args.open_pr;
    options.require_benchmark = run_args.require_benchmark;
    options.allow_protected = run_args.allow_protected;
    options.allow_dirty = run_args.allow_dirty;
    options.include_external = run_args.external;
    options.benchmark_samples = run_args.samples;
    options.report_dir = run_args.report_dir.clone();
    let summary = FleetRunner::new(config).run(options).await?;
    print_fleet_summary(&summary, run_args.json, run_args.sarif)?;
    fleet_exit(&summary)
}

fn print_fleet_summary(
    summary: &codeforge_protocol::FleetRunSummary,
    json: bool,
    sarif: bool,
) -> Result<()> {
    if sarif {
        let diagnostics = fleet_diagnostics(summary)?;
        print_json(&diagnostics_to_sarif(&diagnostics))?;
    } else if json {
        print_json(summary)?;
    } else {
        println!("Fleet:  {}", summary.fleet_name);
        println!("Run:    {}", summary.run_id);
        println!("Status: {:?}", summary.status);
        println!("Report: {}", summary.report_dir.display());
        for repository in &summary.repositories {
            println!(
                "  {:<24} {:<20} findings={:<4} pending={}",
                repository.repository,
                format!("{:?}", repository.status),
                repository.findings,
                repository.pending_transformations
            );
            if let Some(report) = &repository.report_path {
                println!("    report: {}", report.display());
            }
            if !repository.message.is_empty() {
                println!("    {}", repository.message);
            }
        }
    }
    Ok(())
}

fn fleet_diagnostics(
    summary: &codeforge_protocol::FleetRunSummary,
) -> Result<Vec<codeforge_protocol::Diagnostic>> {
    let mut diagnostics = Vec::new();
    for repository in &summary.repositories {
        let Some(evidence) = &repository.evidence else {
            continue;
        };
        let report = evidence.report_dir.join("report.json");
        if !report.is_file() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(report)?)?;
        if let Some(items) = value.get("diagnostics") {
            diagnostics.extend(
                serde_json::from_value::<Vec<codeforge_protocol::Diagnostic>>(items.clone())?,
            );
        }
    }
    Ok(diagnostics)
}

fn fleet_exit(summary: &codeforge_protocol::FleetRunSummary) -> Result<()> {
    use codeforge_protocol::RepoRunStatus;
    if summary.repositories.is_empty() {
        anyhow::bail!("configuration failure: no repositories were scheduled");
    }
    if summary
        .repositories
        .iter()
        .any(|repo| repo.status == RepoRunStatus::VerificationFailure)
    {
        anyhow::bail!("verification failure in fleet run");
    }
    if summary
        .repositories
        .iter()
        .any(|repo| repo.status == RepoRunStatus::MissingTool)
    {
        anyhow::bail!("missing tool in fleet run");
    }
    if summary
        .repositories
        .iter()
        .any(|repo| repo.status == RepoRunStatus::SafetyRefusal)
    {
        anyhow::bail!("safety policy refusal in fleet run");
    }
    if summary
        .repositories
        .iter()
        .any(|repo| repo.status == RepoRunStatus::TransformationFailure)
    {
        anyhow::bail!("transformation failure in fleet run");
    }
    if summary
        .repositories
        .iter()
        .any(|repo| repo.status == RepoRunStatus::ConfigurationFailure)
    {
        anyhow::bail!("configuration failure in fleet run");
    }
    if summary
        .repositories
        .iter()
        .any(|repo| repo.status == RepoRunStatus::Findings)
    {
        anyhow::bail!("findings in fleet run");
    }
    Ok(())
}

fn parse_risk(value: &str) -> Result<codeforge_protocol::RiskLevel> {
    match value.to_ascii_lowercase().as_str() {
        "low" => Ok(codeforge_protocol::RiskLevel::Low),
        "medium" => Ok(codeforge_protocol::RiskLevel::Medium),
        "high" => Ok(codeforge_protocol::RiskLevel::High),
        "very-high" | "very_high" => Ok(codeforge_protocol::RiskLevel::VeryHigh),
        other => anyhow::bail!("invalid risk level: {other}"),
    }
}

fn open(path: &PathBuf) -> Result<CodeForgeEngine> {
    CodeForgeEngine::open(path).with_context(|| format!("cannot open {}", path.display()))
}

fn review_options(args: &CommonArgs) -> ReviewOptions {
    ReviewOptions {
        languages: args
            .languages
            .iter()
            .filter_map(|language| Language::from_name(language).ok())
            .collect(),
        changed_only: args.changed,
        include_external: args.external,
    }
}

fn filter_diagnostics(
    diagnostics: Vec<codeforge_protocol::Diagnostic>,
    engines: &[String],
) -> Vec<codeforge_protocol::Diagnostic> {
    if engines.is_empty() {
        return diagnostics;
    }
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            engines.iter().any(|engine| {
                diagnostic
                    .engine
                    .to_ascii_lowercase()
                    .contains(&engine.to_ascii_lowercase())
            })
        })
        .collect()
}

fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

trait SeverityLabel {
    fn severity_label(&self) -> &'static str;
}

impl SeverityLabel for codeforge_protocol::Diagnostic {
    fn severity_label(&self) -> &'static str {
        match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        }
    }
}
