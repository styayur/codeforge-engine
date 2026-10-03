use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use codeforge_baseline::{BaselineStore, PrecisionRow};
use codeforge_core::{CodeForgeEngine, ReviewOptions};
use codeforge_fleet::{
    Doctor, FleetCommand as FleetOperation, FleetConfig, FleetRunOptions, FleetRunner,
};
use codeforge_protocol::{FindingDisposition, Language, Severity, diagnostics_to_sarif};
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
    /// Inspect local tool availability without installing anything.
    Doctor(DoctorArgs),
    /// Review and maintain the accepted-findings baseline.
    Baseline(BaselineArgs),
    /// Report reviewed-baseline precision metrics.
    Stats(StatsArgs),
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
    /// Include fixture findings in review output.
    #[arg(long)]
    include_fixtures: bool,
    /// Show only findings in the human-review queue.
    #[arg(long)]
    queue: bool,
    /// Optional reviewed baseline for queue and classification output.
    #[arg(long, default_value = ".codeforge/baseline.json")]
    baseline: PathBuf,
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
    /// Reviewed baseline; when supplied, CI fails on new regressions by default.
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Failure policy: `new`, `any`, or `none`.
    #[arg(long, default_value = "new")]
    fail_on: String,
    /// Emit only new findings in SARIF.
    #[arg(long)]
    new_only: bool,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct StatsArgs {
    #[arg(long, default_value = ".codeforge/baseline.json")]
    baseline: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct BaselineArgs {
    #[command(subcommand)]
    command: BaselineCommand,
}

#[derive(Debug, Subcommand)]
enum BaselineCommand {
    Show(BaselineShowArgs),
    Review(BaselineShowArgs),
    Accept(BaselineActionArgs),
    FalsePositive(BaselineActionArgs),
    Unreview(BaselineActionArgs),
    Prune(BaselineShowArgs),
    Migrate(BaselineMigrateArgs),
}

#[derive(Debug, Args)]
struct BaselineShowArgs {
    #[arg(long, default_value = ".codeforge/baseline.json")]
    file: PathBuf,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    path: Option<PathBuf>,
    #[arg(long)]
    include_fixtures: bool,
}

#[derive(Debug, Args)]
struct BaselineActionArgs {
    #[arg(long, default_value = ".codeforge/baseline.json")]
    file: PathBuf,
    #[arg(long, default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    finding: String,
    #[arg(long)]
    reason: String,
}

#[derive(Debug, Args)]
struct BaselineMigrateArgs {
    #[arg(long)]
    from: PathBuf,
    #[arg(long, default_value = ".codeforge/baseline.json")]
    to: PathBuf,
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
    Doctor(FleetDoctorArgs),
    Baseline(FleetBaselineArgs),
}

#[derive(Debug, Args, Clone)]
struct FleetDoctorArgs {
    #[arg(long, default_value = "fleet.toml")]
    config: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args, Clone)]
struct FleetBaselineArgs {
    #[command(subcommand)]
    command: FleetBaselineSubcommand,
}

#[derive(Debug, Subcommand, Clone)]
enum FleetBaselineSubcommand {
    Report(FleetBaselineReportArgs),
}

#[derive(Debug, Args, Clone)]
struct FleetBaselineReportArgs {
    #[arg(long, default_value = "fleet.toml")]
    config: PathBuf,
    #[arg(long, default_value = ".codeforge/baseline.json")]
    baseline: PathBuf,
    #[arg(long)]
    json: bool,
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
        Command::Doctor(args) => doctor(args).await,
        Command::Baseline(args) => baseline(args).await,
        Command::Stats(args) => stats(args).await,
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
    let report = if args.include_fixtures {
        engine
            .review_including_non_production(review_options(&args))
            .await?
    } else {
        engine.review(review_options(&args)).await?
    };
    let diagnostics = filter_diagnostics(report.diagnostics, &args.engines);
    if args.queue {
        return review_queue(&engine, &args, &diagnostics);
    }
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

fn review_queue(
    engine: &CodeForgeEngine,
    args: &CommonArgs,
    diagnostics: &[codeforge_protocol::Diagnostic],
) -> Result<()> {
    let store = BaselineStore::load(resolve_cli_path(engine.root(), &args.baseline))
        .with_context(|| format!("cannot load baseline {}", args.baseline.display()))?;
    let assessments = store.classify_diagnostics(diagnostics, engine.root());
    let mut rows = diagnostics
        .iter()
        .zip(assessments)
        .filter(|(_, assessment)| {
            assessment.disposition == FindingDisposition::HumanReview
                || assessment.lifecycle == codeforge_protocol::FindingLifecycle::Ambiguous
        })
        .map(|(diagnostic, assessment)| {
            serde_json::json!({
                "fingerprint": assessment.fingerprint,
                "rule_id": diagnostic.rule_id,
                "severity": diagnostic.severity,
                "confidence": diagnostic.confidence,
                "source_context": diagnostic.source_context,
                "producer": diagnostic.producer,
                "native_rule_id": diagnostic.native_rule_id,
                "file": diagnostic.file,
                "line": diagnostic.range.start_line,
                "message": diagnostic.message,
                "match_kind": assessment.match_kind,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left["rule_id"]
            .to_string()
            .cmp(&right["rule_id"].to_string())
    });
    if args.json {
        print_json(&rows)?;
    } else if rows.is_empty() {
        println!("Human-review queue is empty.");
    } else {
        println!("Human-review queue: {}", rows.len());
        for row in rows {
            let source_context = row["source_context"].as_str().unwrap_or("unknown");
            let confidence = row["confidence"].as_str().unwrap_or("unknown");
            println!(
                "{} [{}] {}:{} {} ({}, {})",
                row["fingerprint"].as_str().unwrap_or_default(),
                row["rule_id"].as_str().unwrap_or_default(),
                row["file"].as_str().unwrap_or_default(),
                row["line"].as_u64().unwrap_or_default(),
                row["message"].as_str().unwrap_or_default(),
                source_context,
                confidence
            );
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

async fn doctor(args: DoctorArgs) -> Result<()> {
    let report = Doctor::scan(&args.path).await;
    if args.json {
        print_json(&serde_json::json!({
            "root": report.root,
            "entries": report.entries,
            "gaps": report.gaps,
        }))?;
    } else {
        println!("Toolchain doctor: {}", report.root.display());
        for entry in &report.entries {
            println!(
                "{:<24} {:<8} {:<20} {}",
                entry.tool,
                entry.status,
                entry.version.as_deref().unwrap_or("unknown version"),
                entry
                    .path
                    .as_deref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "not found".to_owned())
            );
            println!("  required for: {}", entry.required_for);
            if let Some(hint) = &entry.install_hint {
                println!("  hint: {hint}");
            }
        }
        println!("{} tool gap(s)", report.gaps.len());
    }
    Ok(())
}

async fn baseline(args: BaselineArgs) -> Result<()> {
    match args.command {
        BaselineCommand::Show(args) => show_baseline(args, false).await?,
        BaselineCommand::Review(args) => show_baseline(args, true).await?,
        BaselineCommand::Accept(args) => {
            update_baseline(
                &args.file,
                &args.path,
                &args.finding,
                FindingDisposition::Accepted,
                Some(args.reason),
            )
            .await?;
        }
        BaselineCommand::FalsePositive(args) => {
            update_baseline(
                &args.file,
                &args.path,
                &args.finding,
                FindingDisposition::FalsePositive,
                Some(args.reason),
            )
            .await?;
        }
        BaselineCommand::Unreview(args) => {
            update_baseline(
                &args.file,
                &args.path,
                &args.finding,
                FindingDisposition::Unreviewed,
                None,
            )
            .await?;
        }
        BaselineCommand::Prune(args) => {
            let mut store = BaselineStore::load(&args.file)?;
            let removed = store.prune();
            store.save(&args.file)?;
            if args.json {
                print_json(&serde_json::json!({ "removed": removed }))?;
            } else {
                println!("Removed {removed} resolved/stale baseline entries.");
            }
        }
        BaselineCommand::Migrate(args) => {
            let (store, report) = BaselineStore::migrate_legacy(&args.from)?;
            store.save(&args.to)?;
            print_json(&report)?;
        }
    }
    Ok(())
}

async fn show_baseline(args: BaselineShowArgs, review: bool) -> Result<()> {
    let store = BaselineStore::load(&args.file)
        .with_context(|| format!("cannot load baseline {}", args.file.display()))?;
    if !review {
        if args.json {
            print_json(&store.entries())?;
        } else if store.entries().is_empty() {
            println!("No baseline entries.");
        } else {
            for entry in store.entries() {
                println!(
                    "{} [{}] {}:{} {:?}",
                    entry.fingerprint,
                    entry.rule_id,
                    entry.path.display(),
                    entry.symbol.as_deref().unwrap_or("-"),
                    entry.disposition
                );
                if let Some(reason) = &entry.reason {
                    println!("  reason: {reason}");
                }
            }
        }
        return Ok(());
    }

    let repository = args.path.clone().unwrap_or_else(|| PathBuf::from("."));
    let engine = open(&repository)?;
    let options = ReviewOptions {
        languages: Vec::new(),
        changed_only: false,
        include_external: true,
    };
    let report = if args.include_fixtures {
        engine.review_including_non_production(options).await?
    } else {
        engine.review(options).await?
    };
    let assessments = store.classify_diagnostics(&report.diagnostics, engine.root());
    let mut rows = report
        .diagnostics
        .iter()
        .zip(assessments)
        .filter(|(_, assessment)| {
            assessment.match_kind == codeforge_protocol::BaselineMatchKind::New
                || assessment.disposition == FindingDisposition::HumanReview
                || assessment.lifecycle == codeforge_protocol::FindingLifecycle::Ambiguous
        })
        .map(|(diagnostic, assessment)| {
            serde_json::json!({
                "fingerprint": assessment.fingerprint,
                "rule_id": diagnostic.rule_id,
                "severity": diagnostic.severity,
                "confidence": diagnostic.confidence,
                "source_context": diagnostic.source_context,
                "file": diagnostic.file,
                "line": diagnostic.range.start_line,
                "message": diagnostic.message,
                "match_kind": assessment.match_kind,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left["rule_id"]
            .to_string()
            .cmp(&right["rule_id"].to_string())
    });
    if args.json {
        print_json(&rows)?;
    } else if rows.is_empty() {
        println!("No unreviewed or human-review findings.");
    } else {
        for row in rows {
            println!(
                "{} [{}] {}:{} {} ({}, {})",
                row["fingerprint"].as_str().unwrap_or_default(),
                row["rule_id"].as_str().unwrap_or_default(),
                row["file"].as_str().unwrap_or_default(),
                row["line"].as_u64().unwrap_or_default(),
                row["message"].as_str().unwrap_or_default(),
                row["source_context"].as_str().unwrap_or("unknown"),
                row["confidence"].as_str().unwrap_or("unknown")
            );
        }
    }
    Ok(())
}

async fn update_baseline(
    path: &PathBuf,
    repository: &PathBuf,
    finding: &str,
    disposition: FindingDisposition,
    reason: Option<String>,
) -> Result<()> {
    let mut store = if path.exists() {
        BaselineStore::load(path)?
    } else {
        BaselineStore::new()
    };
    if !store.set_disposition(finding, disposition, reason.clone())? {
        let engine = open(repository)?;
        let report = engine
            .review_including_non_production(review_options(&CommonArgs {
                path: repository.clone(),
                json: false,
                sarif: false,
                changed: false,
                languages: Vec::new(),
                engines: Vec::new(),
                external: true,
                include_fixtures: true,
                queue: false,
                baseline: path.clone(),
            }))
            .await?;
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|diagnostic| {
                let fingerprint =
                    codeforge_baseline::fingerprint_diagnostic(diagnostic, engine.root());
                fingerprint.exact == finding
            })
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("finding fingerprint not found: {finding}"))?;
        store.review(&diagnostic, engine.root(), disposition, reason)?;
    }
    store.save(path)?;
    println!("Updated {finding} to {disposition}.");
    Ok(())
}

async fn stats(args: StatsArgs) -> Result<()> {
    let store = BaselineStore::load(&args.baseline)?;
    let rows = store.precision_stats();
    if args.json {
        print_json(&rows)?;
    } else {
        print_precision_rows(&rows);
    }
    Ok(())
}

fn print_precision_rows(rows: &[PrecisionRow]) {
    println!(
        "{:<28} {:>9} {:>9} {:>9} {:>9}",
        "rule", "reviewed", "accepted", "false+", "human"
    );
    for row in rows {
        println!(
            "{:<28} {:>9} {:>9} {:>9} {:>9}",
            row.rule_id, row.reviewed, row.accepted, row.false_positive, row.human_review
        );
    }
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
    let report = engine
        .review(ReviewOptions {
            languages: Vec::new(),
            changed_only: args.changed,
            include_external: true,
        })
        .await?;
    let verification = engine.verify(true).await?;
    let baseline_path = args
        .baseline
        .as_ref()
        .map(|path| resolve_cli_path(engine.root(), path));
    let comparison = if let Some(path) = &baseline_path {
        let store = BaselineStore::load(path)
            .with_context(|| format!("cannot load baseline {}", path.display()))?;
        Some(store.compare(&report.diagnostics, engine.root()))
    } else {
        None
    };
    let assessments = if let Some(path) = &baseline_path {
        let store = BaselineStore::load(path)?;
        store.classify_diagnostics(&report.diagnostics, engine.root())
    } else {
        Vec::new()
    };
    if let Some(path) = &args.sarif {
        let diagnostics = if args.new_only {
            let new_fingerprints = comparison
                .as_ref()
                .map(|comparison| {
                    comparison
                        .new
                        .iter()
                        .map(|entry| entry.fingerprint.as_str())
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .unwrap_or_default();
            report
                .diagnostics
                .iter()
                .zip(&assessments)
                .filter(|(_, assessment)| {
                    new_fingerprints.contains(assessment.fingerprint.as_str())
                })
                .map(|(diagnostic, _)| diagnostic.clone())
                .collect::<Vec<_>>()
        } else {
            report.diagnostics.clone()
        };
        let sarif = diagnostics_to_sarif(&diagnostics);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&sarif)?)?;
    }
    if args.json {
        print_json(&serde_json::json!({
            "diagnostics": report.diagnostics,
            "assessments": assessments,
            "baseline": comparison,
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
    match (comparison.as_ref(), args.fail_on.as_str()) {
        (Some(comparison), "new") if !comparison.new.is_empty() => {
            anyhow::bail!("new regressions");
        }
        (Some(_), "none") => {}
        (Some(_), "any") if !report.diagnostics.is_empty() => {
            anyhow::bail!("findings");
        }
        (Some(_), "new" | "any") => {}
        (None, _) if !report.diagnostics.is_empty() => {
            anyhow::bail!("findings");
        }
        (None, "new" | "any" | "none") => {}
        (_, policy) => {
            anyhow::bail!("invalid --fail-on policy: {policy}");
        }
    }
    Ok(())
}

async fn fleet(args: FleetArgs) -> Result<()> {
    if let FleetSubcommand::Doctor(args) = &args.command {
        return fleet_doctor(args).await;
    }
    if let FleetSubcommand::Baseline(args) = &args.command {
        return fleet_baseline_report(args).await;
    }
    let (operation, run_args) = match &args.command {
        FleetSubcommand::Audit(args) => (FleetOperation::Audit, args),
        FleetSubcommand::Format(args) => (FleetOperation::Format, args),
        FleetSubcommand::Review(args) => (FleetOperation::Review, args),
        FleetSubcommand::Refactor(args) => (FleetOperation::Refactor, args),
        FleetSubcommand::Optimize(args) => (FleetOperation::Optimize, args),
        FleetSubcommand::Verify(args) => (FleetOperation::Verify, args),
        FleetSubcommand::Report(args) => (FleetOperation::Report, args),
        FleetSubcommand::Doctor(_) | FleetSubcommand::Baseline(_) => unreachable!(),
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

async fn fleet_doctor(args: &FleetDoctorArgs) -> Result<()> {
    let config = FleetConfig::load(&args.config)?;
    let mut reports = Vec::new();
    for repository in config.repositories()? {
        let report = Doctor::scan(&repository.path).await;
        reports.push(serde_json::json!({
            "repository": repository.name,
            "path": repository.path,
            "entries": report.entries,
            "gaps": report.gaps,
        }));
    }
    if args.json {
        print_json(&reports)?;
    } else {
        for report in reports {
            let repository = report["repository"].as_str().unwrap_or_default();
            println!("## {repository}");
            let entries = report["entries"].as_array().cloned().unwrap_or_default();
            for entry in entries {
                println!(
                    "  {:<24} {}",
                    entry["tool"].as_str().unwrap_or_default(),
                    entry["status"].as_str().unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}

async fn fleet_baseline_report(args: &FleetBaselineArgs) -> Result<()> {
    let FleetBaselineSubcommand::Report(report_args) = &args.command;
    let _config = FleetConfig::load(&report_args.config)?;
    let store = BaselineStore::load(&report_args.baseline)?;
    let rows = store.precision_stats();
    let summary = serde_json::json!({
        "entries": store.entries().len(),
        "precision": rows,
    });
    if report_args.json {
        print_json(&summary)?;
    } else {
        println!("Baseline entries: {}", store.entries().len());
        print_precision_rows(&rows);
    }
    Ok(())
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
        println!("Execution: {:?}", summary.execution_status);
        println!("Findings:  {:?}", summary.finding_status);
        println!("Source findings: {}", summary.source_findings);
        println!("Tool gaps: {}", summary.tool_gaps.len());
        println!("Verification failures: {}", summary.verification_failures);
        println!("Configuration failures: {}", summary.configuration_failures);
        println!("Report: {}", summary.report_dir.display());
        for repository in &summary.repositories {
            println!(
                "  {:<24} {:<20} source={:<4} gaps={:<3} pending={}",
                repository.repository,
                format!("{:?}", repository.status),
                repository.source_findings.max(repository.findings),
                repository.tool_gaps.len(),
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

fn resolve_cli_path(root: &std::path::Path, path: &std::path::Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else if root.join(path).exists() {
        root.join(path)
    } else if let Ok(current_dir) = std::env::current_dir() {
        current_dir.join(path)
    } else {
        root.join(path)
    }
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
