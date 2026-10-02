use std::path::{Path, PathBuf};

use codeforge_protocol::{
    BenchmarkResult, Diagnostic, EvidenceBundle, Patch, RiskLevel, TransformationClass,
    VerificationEvidence, VerificationResult, diagnostics_to_sarif,
};
use serde::{Deserialize, Serialize};

use crate::FleetError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSnapshot {
    pub repository: PathBuf,
    pub diagnostics: usize,
    pub verification: VerificationResult,
    pub files: Vec<PathBuf>,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct EvidenceInput {
    pub id: String,
    pub repository: PathBuf,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub risk: RiskLevel,
    pub transformation_classes: Vec<TransformationClass>,
    pub baseline: VerificationResult,
    pub after: VerificationResult,
    pub benchmark: Option<BenchmarkResult>,
    pub patch: Option<Patch>,
    pub diagnostics: Vec<Diagnostic>,
    pub before_snapshot: EvidenceSnapshot,
    pub after_snapshot: EvidenceSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrittenEvidence {
    pub bundle: EvidenceBundle,
    pub report_markdown: PathBuf,
    pub report_json: PathBuf,
    pub sarif: PathBuf,
    pub patch_diff: PathBuf,
    pub before_json: PathBuf,
    pub after_json: PathBuf,
}

#[derive(Debug, Clone)]
pub struct EvidenceWriter {
    root: PathBuf,
}

impl EvidenceWriter {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn write(&self, input: EvidenceInput) -> Result<WrittenEvidence, FleetError> {
        let report_dir = self.root.join(&input.id);
        std::fs::create_dir_all(&report_dir)?;
        let evidence = VerificationEvidence::from_result(&input.after);
        let bundle = EvidenceBundle {
            id: input.id.clone(),
            repository: input.repository.clone(),
            commit: input.commit.clone(),
            branch: input.branch.clone(),
            risk: input.risk,
            transformation_classes: input.transformation_classes.clone(),
            baseline: input.baseline.clone(),
            after: input.after.clone(),
            evidence,
            benchmark: input.benchmark.clone(),
            patch: input.patch.clone(),
            report_dir: report_dir.clone(),
        };

        let report_markdown = report_dir.join("report.md");
        let report_json = report_dir.join("report.json");
        let sarif = report_dir.join("diagnostics.sarif");
        let patch_diff = report_dir.join("patch.diff");
        let before_json = report_dir.join("before.json");
        let after_json = report_dir.join("after.json");

        atomic_write(
            &report_markdown,
            render_markdown(
                &bundle,
                &input.diagnostics,
                &input.before_snapshot,
                &input.after_snapshot,
            )
            .as_bytes(),
        )?;
        atomic_write(
            &report_json,
            serde_json::to_vec_pretty(&serde_json::json!({
                "bundle": bundle,
                "diagnostics": input.diagnostics,
                "before": input.before_snapshot,
                "after": input.after_snapshot,
            }))?
            .as_slice(),
        )?;
        atomic_write(
            &sarif,
            serde_json::to_vec_pretty(&diagnostics_to_sarif(&input.diagnostics))?.as_slice(),
        )?;
        let patch_text = input
            .patch
            .as_ref()
            .map(render_patch)
            .unwrap_or_else(|| "# no patch generated\n".to_owned());
        atomic_write(&patch_diff, patch_text.as_bytes())?;
        atomic_write(
            &before_json,
            serde_json::to_vec_pretty(&input.before_snapshot)?.as_slice(),
        )?;
        atomic_write(
            &after_json,
            serde_json::to_vec_pretty(&input.after_snapshot)?.as_slice(),
        )?;

        Ok(WrittenEvidence {
            bundle,
            report_markdown,
            report_json,
            sarif,
            patch_diff,
            before_json,
            after_json,
        })
    }

    pub fn latest(&self) -> Result<Option<PathBuf>, FleetError> {
        if !self.root.exists() {
            return Ok(None);
        }
        let mut reports = std::fs::read_dir(&self.root)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.join("report.json").is_file())
            .collect::<Vec<_>>();
        reports.sort();
        Ok(reports.pop())
    }
}

fn render_markdown(
    bundle: &EvidenceBundle,
    diagnostics: &[Diagnostic],
    before: &EvidenceSnapshot,
    after: &EvidenceSnapshot,
) -> String {
    let classes = if bundle.transformation_classes.is_empty() {
        "none".to_owned()
    } else {
        bundle
            .transformation_classes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut output = format!(
        "# CodeForge Transformation Report\n\nRepository: `{}`\n\nCommit: {}\n\nBranch: {}\n\nRun: `{}`\n\n## Baseline\n\n{}\n\n## Findings\n\n{} diagnostics\n\n## Changes\n\n",
        bundle.repository.display(),
        bundle.commit.as_deref().unwrap_or("unknown"),
        bundle.branch.as_deref().unwrap_or("unknown"),
        bundle.id,
        render_verification(&bundle.baseline),
        diagnostics.len(),
    );
    if let Some(patch) = &bundle.patch {
        output.push_str(&format!(
            "{} files\n{} additions\n{} deletions\n\n",
            patch.files.len(),
            patch.additions,
            patch.deletions
        ));
    } else {
        output.push_str("No source patch was generated.\n\n");
    }
    output.push_str("## Verification\n\n");
    output.push_str(&render_verification(&bundle.after));
    output.push_str("\n\n## Performance\n\n");
    if let Some(benchmark) = &bundle.benchmark {
        output.push_str(&format!(
            "Benchmark available: yes\n\nBefore median: {:.3} {}\n\nAfter median: {:.3} {}\n\nDelta: {:+.2}%\n\nSamples: {}/{}\n\nWarmup: {}\n\n",
            benchmark.before.median,
            benchmark.before.unit,
            benchmark.after.median,
            benchmark.after.unit,
            benchmark.delta_percent,
            benchmark.before.samples.len(),
            benchmark.after.samples.len(),
            benchmark.warmup
        ));
    } else {
        output.push_str("Benchmark available: no. No performance claim is made.\n\n");
    }
    output.push_str(&format!(
        "## Risk\n\n{}\n\n## Transformation classes\n\n{}\n\n## Evidence\n\n{}\n\n## Rollback\n\nTransaction rollback is recorded in `.codeforge/history` when a source transaction is applied. This report directory is {}. \n\n## Snapshot notes\n\nBefore: {}\n\nAfter: {}\n",
        format!("{:?}", bundle.risk).to_ascii_uppercase(),
        classes,
        bundle
            .evidence
            .levels()
            .into_iter()
            .map(|level| format!("{level:?}").to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join(", "),
        bundle.report_dir.display(),
        before.note,
        after.note,
    ));
    output
}

fn render_verification(result: &VerificationResult) -> String {
    [
        ("Format", &result.syntax),
        ("Typecheck", &result.typecheck),
        ("Build", &result.build),
        ("Tests", &result.tests),
        ("Benchmark", &result.benchmark),
        ("Equivalence", &result.equivalence),
    ]
    .into_iter()
    .map(|(label, check)| format!("{label}: {:?}", check.status).to_ascii_uppercase())
    .collect::<Vec<_>>()
    .join("\n")
}

fn render_patch(patch: &Patch) -> String {
    let mut output = String::new();
    for file in &patch.files {
        output.push_str(&format!("# {}\n", file.file.display()));
        output.push_str(&file.unified_diff);
        if !file.unified_diff.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), FleetError> {
    let parent = path.parent().ok_or_else(|| {
        FleetError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("path has no parent: {}", path.display()),
        ))
    })?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("report"),
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeforge_protocol::{VerificationCheck, VerificationStatus};

    #[test]
    fn writes_complete_evidence_bundle() {
        let temp = tempfile::tempdir().expect("tempdir");
        let baseline = VerificationResult {
            syntax: VerificationCheck::passed("ok", 1),
            ..VerificationResult::default()
        };
        let after = VerificationResult {
            syntax: VerificationCheck::passed("ok", 1),
            tests: VerificationCheck::passed("ok", 1),
            ..VerificationResult::default()
        };
        let snapshot = EvidenceSnapshot {
            repository: temp.path().to_path_buf(),
            diagnostics: 0,
            verification: baseline.clone(),
            files: Vec::new(),
            note: "snapshot".to_owned(),
        };
        let writer = EvidenceWriter::new(temp.path().join("reports"));
        let written = writer
            .write(EvidenceInput {
                id: "run-1".to_owned(),
                repository: temp.path().to_path_buf(),
                commit: Some("abc".to_owned()),
                branch: Some("main".to_owned()),
                risk: RiskLevel::Low,
                transformation_classes: vec![TransformationClass::StyleOnly],
                baseline,
                after,
                benchmark: None,
                patch: None,
                diagnostics: Vec::new(),
                before_snapshot: snapshot.clone(),
                after_snapshot: snapshot,
            })
            .expect("evidence");
        assert_eq!(written.bundle.evidence.tested, VerificationStatus::Passed);
        assert!(written.report_markdown.is_file());
        assert!(written.sarif.is_file());
        assert!(written.patch_diff.is_file());
    }
}
