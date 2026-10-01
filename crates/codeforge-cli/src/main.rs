use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use codeforge_core::{CodeForgeEngine, ReviewOptions};
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
    /// Show safe and unsafe transformations proposed by built-in adapters.
    Refactor(CommonArgs),
    /// Generate a performance candidate, verify it in a sandbox, and benchmark it.
    Optimize(CommonArgs),
    /// Run syntax checks plus configured/local build and test verification.
    Verify(CommonArgs),
    /// Run a configured or auto-detected benchmark command.
    Benchmark(BenchmarkArgs),
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Scan(args) => scan(args).await,
        Command::Lint(args) | Command::Review(args) => review(args).await,
        Command::Refactor(args) => refactor(args).await,
        Command::Optimize(args) => optimize(args).await,
        Command::Verify(args) => verify(args).await,
        Command::Benchmark(args) => benchmark(args).await,
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
