use std::hint::black_box;
use std::path::PathBuf;
use std::time::Duration;

use codeforge_core::{CodeForgeEngine, ReviewOptions};
use codeforge_diagnostics::DiagnosticsAggregator;
use codeforge_engines::{EngineRegistry, EngineRequest, analyze_source};
use codeforge_protocol::{
    Confidence, Diagnostic, DiagnosticCategory, Language, Severity, SourceRange,
};
use codeforge_workspace::WorkspaceManager;
use criterion::{Criterion, criterion_group, criterion_main};

fn repository(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures")
        .join(name)
}

fn benchmark_startup(c: &mut Criterion) {
    let root = repository("python");
    c.bench_function("startup_open_workspace", |b| {
        b.iter(|| black_box(WorkspaceManager::open(black_box(&root)).expect("workspace")));
    });
}

fn benchmark_indexing_1k(c: &mut Criterion) {
    let temp = tempfile::tempdir().expect("tempdir");
    for index in 0..1000 {
        std::fs::write(
            temp.path().join(format!("file_{index:04}.rs")),
            "pub fn value() -> usize { 1 }\n",
        )
        .expect("fixture file");
    }
    c.bench_function("repository_index_1k_files", |b| {
        b.iter(|| black_box(WorkspaceManager::open(black_box(temp.path())).expect("workspace")));
    });
}

fn benchmark_single_file_analysis(c: &mut Criterion) {
    let source = r#"
struct Service { values: Vec<u32> }
impl Service {
    fn unused(&self) -> usize {
        self.values.len() == 0
    }
}
"#;
    c.bench_function("single_file_tree_sitter_analysis", |b| {
        b.iter(|| {
            black_box(analyze_source(
                "rust-builtin",
                Language::Rust,
                std::path::Path::new("service.rs"),
                black_box(source),
            ))
        });
    });
}

fn benchmark_ast_search(c: &mut Criterion) {
    let source = include_str!("../../../fixtures/go/fixable.go");
    let registry = EngineRegistry::with_builtin_adapters();
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    c.bench_function("ast_search_go_fixture", |b| {
        b.iter(|| {
            runtime.block_on(async {
                black_box(
                    registry
                        .analyze(&EngineRequest::new(
                            repository("go"),
                            vec![repository("go").join("fixable.go")],
                        ))
                        .await,
                )
            });
            black_box(source)
        });
    });
}

fn benchmark_aggregation(c: &mut Criterion) {
    let diagnostics = (0..2000)
        .map(|index| {
            Diagnostic::new(
                if index % 2 == 0 {
                    "engine-a"
                } else {
                    "engine-b"
                },
                Language::Rust,
                format!("RULE-{}", index % 50),
                Severity::Warning,
                DiagnosticCategory::Correctness,
                Confidence::Medium,
                PathBuf::from("src/main.rs"),
                SourceRange::new(index, index + 1, 1, 1, 1, 2).expect("range"),
                format!("finding {}", index % 50),
            )
        })
        .collect::<Vec<_>>();
    c.bench_function("diagnostic_aggregation_2k", |b| {
        b.iter(|| {
            let mut aggregator = DiagnosticsAggregator::new();
            aggregator.extend(black_box(diagnostics.clone()));
            black_box(aggregator.aggregate())
        });
    });
}

fn benchmark_incremental_analysis(c: &mut Criterion) {
    let engine = CodeForgeEngine::open(repository("rust")).expect("engine");
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    c.bench_function("incremental_review_cached_workspace", |b| {
        b.iter(|| {
            runtime.block_on(async {
                black_box(
                    engine
                        .review(ReviewOptions {
                            languages: vec![Language::Rust],
                            changed_only: false,
                            include_external: false,
                        })
                        .await
                        .expect("review"),
                )
            })
        });
    });
}

fn configured_criterion() -> Criterion {
    Criterion::default()
        .sample_size(20)
        .measurement_time(Duration::from_secs(5))
}

criterion_group! {
    name = benches;
    config = configured_criterion();
    targets =
        benchmark_startup,
        benchmark_indexing_1k,
        benchmark_single_file_analysis,
        benchmark_incremental_analysis,
        benchmark_ast_search,
        benchmark_aggregation
}
criterion_main!(benches);
