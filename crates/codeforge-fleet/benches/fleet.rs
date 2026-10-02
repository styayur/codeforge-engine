use std::time::Duration;

use codeforge_fleet::{
    EvidenceInput, EvidenceSnapshot, EvidenceWriter, FleetCommand, FleetConfig, FleetRunOptions,
    FleetRunner, ProjectDetector,
};
use codeforge_protocol::{RiskLevel, TransformationClass, VerificationCheck, VerificationResult};
use criterion::{Criterion, criterion_group, criterion_main};

fn fleet_benchmarks(criterion: &mut Criterion) {
    let root = std::env::current_dir().expect("cwd");
    let config_path = root.join("fleet.toml");
    if !config_path.exists() {
        return;
    }
    let config = FleetConfig::load(config_path).expect("fleet config");
    let runner = FleetRunner::new(config);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    criterion.bench_function("fleet discovery and audit", |bencher| {
        bencher.iter(|| {
            runtime
                .block_on(runner.run(FleetRunOptions::new(FleetCommand::Audit)))
                .expect("fleet audit");
        });
    });
    criterion.bench_function("project detection", |bencher| {
        bencher.iter(|| ProjectDetector::detect(&root).expect("project detection"));
    });
    criterion.bench_function("report generation", |bencher| {
        let verification = VerificationResult {
            syntax: VerificationCheck::passed("ok", 1),
            ..VerificationResult::default()
        };
        let snapshot = EvidenceSnapshot {
            repository: root.clone(),
            diagnostics: 0,
            verification: verification.clone(),
            files: Vec::new(),
            note: "benchmark".to_owned(),
        };
        let writer = EvidenceWriter::new(std::env::temp_dir().join("codeforge-fleet-bench"));
        let mut sequence = 0usize;
        bencher.iter(|| {
            sequence += 1;
            writer
                .write(EvidenceInput {
                    id: format!("benchmark-{sequence}"),
                    repository: root.clone(),
                    commit: None,
                    branch: None,
                    risk: RiskLevel::Low,
                    transformation_classes: vec![TransformationClass::StyleOnly],
                    baseline: verification.clone(),
                    after: verification.clone(),
                    benchmark: None,
                    patch: None,
                    diagnostics: Vec::new(),
                    before_snapshot: snapshot.clone(),
                    after_snapshot: snapshot.clone(),
                })
                .expect("report generation");
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = fleet_benchmarks
}
criterion_main!(benches);
