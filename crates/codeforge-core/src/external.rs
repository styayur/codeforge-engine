//! Opt-in local CLI analysis and SARIF evidence. Never participates in default review.
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use codeforge_protocol::{
    Diagnostic, Language, SarifLocation, SarifLog, SourceRange, sarif_to_diagnostics,
};
use codeforge_verification::{CommandSpec, VerificationPipeline};
use serde::Serialize;
use serde_json::Value;

use crate::CoreError;

const MAX_SARIF_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisStep {
    pub file: PathBuf,
    pub range: SourceRange,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisPath {
    /// Index into this report's diagnostics; independent of random diagnostic IDs.
    pub diagnostic_index: usize,
    pub steps: Vec<AnalysisStep>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExternalAnalysis {
    pub evidence: &'static str,
    pub diagnostics: Vec<Diagnostic>,
    pub analysis_paths: Vec<AnalysisPath>,
    pub warnings: Vec<String>,
}

/// Import evidence only. SARIF fixes are never granted application authority.
pub fn import_sarif(
    path: &Path,
    root: &Path,
    language: Language,
) -> Result<ExternalAnalysis, CoreError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_SARIF_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    import_bytes(&bytes, root, language)
}

fn import_bytes(
    bytes: &[u8],
    root: &Path,
    language: Language,
) -> Result<ExternalAnalysis, CoreError> {
    if bytes.len() > MAX_SARIF_BYTES {
        return Err(invalid("SARIF exceeds 16 MiB"));
    }
    let mut raw: Value = serde_json::from_slice(bytes).map_err(|e| invalid(&e.to_string()))?;
    if raw.get("version").and_then(Value::as_str) != Some("2.1.0") {
        return Err(invalid("expected SARIF 2.1.0"));
    }
    raw.as_object_mut()
        .ok_or_else(|| invalid("expected SARIF object"))?
        .entry("$schema")
        .or_insert_with(|| Value::String("https://json.schemastore.org/sarif-2.1.0.json".into()));
    let log: SarifLog = serde_json::from_value(raw.clone()).map_err(|e| invalid(&e.to_string()))?;
    validate_uri_bases(&raw)?;
    let mut diagnostics =
        sarif_to_diagnostics(&log, language).map_err(|e| invalid(&e.to_string()))?;
    if diagnostics.len() > 10_000 {
        return Err(invalid("SARIF exceeds 10,000 findings"));
    }
    let root = root.canonicalize()?;
    for diagnostic in &mut diagnostics {
        diagnostic.file = source_path(&diagnostic.file.to_string_lossy(), &root)?;
        if diagnostic.range.start_line == 0 || diagnostic.range.start_column == 0 {
            return Err(invalid("source positions must be one-based"));
        }
        diagnostic.evidence.push("static-analysis: imported external SARIF; not compilation, test or semantic verification".into());
        diagnostic.producer = Some(diagnostic.engine.clone());
        diagnostic.fixes.clear();
        diagnostic.safe_fix_available = false;
    }
    let mut analysis_paths = Vec::new();
    let mut warnings = Vec::new();
    let mut diagnostic_index = 0;
    let mut total_steps = 0;
    for run in raw["runs"]
        .as_array()
        .ok_or_else(|| invalid("missing runs"))?
    {
        for result in run
            .get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for flow in result
                .get("codeFlows")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                for thread in flow
                    .get("threadFlows")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let mut steps = Vec::new();
                    for point in thread
                        .get("locations")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        total_steps += 1;
                        if total_steps > 100_000 {
                            return Err(invalid("SARIF exceeds 100,000 path steps"));
                        }
                        let Some(location) = point.get("location") else {
                            return Err(invalid("path step has no location"));
                        };
                        let location: SarifLocation = match serde_json::from_value(location.clone())
                        {
                            Ok(location) => location,
                            Err(_) => {
                                warnings.push(format!("finding {diagnostic_index}: path step lacks a supported physical source location"));
                                continue;
                            }
                        };
                        let physical = location.physical_location;
                        let region = physical.region;
                        if region.start_line == 0 || region.start_column == 0 {
                            return Err(invalid("path position must be one-based"));
                        }
                        steps.push(AnalysisStep {
                            file: source_path(&physical.artifact_location.uri, &root)?,
                            range: SourceRange::new(
                                0,
                                0,
                                region.start_line,
                                region.start_column,
                                region.end_line.max(region.start_line),
                                if region.end_column == 0 {
                                    region.start_column
                                } else {
                                    region.end_column
                                },
                            )
                            .map_err(|e| invalid(&e.to_string()))?,
                            message: point
                                .pointer("/location/message/text")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        });
                    }
                    analysis_paths.push(AnalysisPath {
                        diagnostic_index,
                        steps,
                    });
                }
            }
            diagnostic_index += 1;
        }
    }
    Ok(ExternalAnalysis {
        evidence: "static-analysis",
        diagnostics,
        analysis_paths,
        warnings,
    })
}

/// Analyze an existing database with explicit local query inputs. No discovery/install.
pub async fn analyze_codeql(
    program: &Path,
    database: &Path,
    queries: &Path,
    root: &Path,
    language: Language,
    timeout_secs: u64,
) -> Result<ExternalAnalysis, CoreError> {
    if !program.is_absolute() || !program.is_file() {
        return Err(invalid(
            "supply an absolute path to an installed CodeQL CLI",
        ));
    }
    if !database.is_dir() || !queries.exists() {
        return Err(invalid(
            "database and query inputs must already exist locally",
        ));
    }
    if timeout_secs == 0 || timeout_secs > 3600 {
        return Err(invalid("timeout must be 1..=3600 seconds"));
    }
    let output = tempfile::tempdir()?;
    let sarif = output.path().join("analysis.sarif");
    let spec = CommandSpec::new(
        program,
        vec![
            "database".into(),
            "analyze".into(),
            database.canonicalize()?.to_string_lossy().into_owned(),
            queries.canonicalize()?.to_string_lossy().into_owned(),
            "--format=sarif-latest".into(),
            format!("--output={}", sarif.display()),
            "--threads=1".into(),
            "--ram=1024".into(),
        ],
        root.canonicalize()?,
    )
    .timeout(Duration::from_secs(timeout_secs))
    .max_output_bytes(1024 * 1024)
    .label("codeql");
    let process = VerificationPipeline.run_command(spec).await?;
    if !process.success() || process.output_truncated {
        return Err(invalid(
            "CodeQL failed, timed out or exceeded output bounds; no evidence imported",
        ));
    }
    tracing::info!(
        tool = "codeql",
        elapsed_ms = process.elapsed.as_millis(),
        "external analysis completed"
    );
    import_sarif(&sarif, root, language)
}

fn invalid(message: &str) -> CoreError {
    CoreError::InvalidProject(format!("external analysis: {message}"))
}

fn validate_uri_bases(value: &Value) -> Result<(), CoreError> {
    match value {
        Value::Object(fields) => {
            if let Some(base) = fields.get("uriBaseId") {
                if base.as_str() != Some("%SRCROOT%") {
                    return Err(invalid(
                        "unsupported URI base; use repository-relative URIs or %SRCROOT%",
                    ));
                }
            }
            for value in fields.values() {
                validate_uri_bases(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_uri_bases(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn source_path(uri: &str, root: &Path) -> Result<PathBuf, CoreError> {
    let bytes = uri.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(invalid("invalid percent encoding"));
            }
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|_| invalid("invalid URI"))?;
            decoded.push(u8::from_str_radix(hex, 16).map_err(|_| invalid("invalid URI escape"))?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let text = String::from_utf8(decoded)
        .map_err(|_| invalid("URI is not UTF-8"))?
        .replace('\\', "/");
    let text = text.strip_prefix("file://").unwrap_or(&text);
    #[cfg(windows)]
    let text = if text.starts_with('/') && text.as_bytes().get(2) == Some(&b':') {
        &text[1..]
    } else {
        text
    };
    let path = PathBuf::from(text);
    let path = if path.is_absolute() {
        let canonical = path
            .canonicalize()
            .map_err(|_| invalid("absolute source URI is unavailable"))?;
        canonical
            .strip_prefix(root)
            .map_err(|_| invalid("source URI escapes repository"))?
            .to_path_buf()
    } else {
        path
    };
    if path.as_os_str().is_empty()
        || path.to_string_lossy().contains([':', '\0'])
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(invalid(
            "source URI must identify a repository-relative path",
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(uri: &str) -> Value {
        let location = serde_json::json!({"physicalLocation":{"artifactLocation":{"uri":uri},"region":{"startLine":2}}});
        serde_json::json!({"version":"2.1.0","runs":[{"tool":{"driver":{"name":"CodeQL","semanticVersion":"test"}},"results":[{"ruleId":"test/path","message":{"text":"flow"},"locations":[location.clone()],"codeFlows":[{"threadFlows":[{"locations":[{"location":location}]}]}]}]}]})
    }

    #[test]
    fn standard_codeql_sarif_import_keeps_path_evidence_and_no_fix_authority() {
        let root = tempfile::tempdir().expect("root");
        let report = import_bytes(
            &serde_json::to_vec(&fixture("src/a%20b.rs")).expect("json"),
            root.path(),
            Language::Rust,
        )
        .expect("import");
        assert_eq!(report.evidence, "static-analysis");
        assert_eq!(report.diagnostics[0].file, PathBuf::from("src/a b.rs"));
        assert_eq!(report.analysis_paths[0].steps[0].range.start_line, 2);
        assert_eq!(report.analysis_paths[0].diagnostic_index, 0);
        assert!(!report.diagnostics[0].safe_fix_available);
        assert!(report.diagnostics[0].fixes.is_empty());
    }

    #[test]
    fn rejects_traversal_schemes_malformed_and_oversize_evidence() {
        let root = tempfile::tempdir().expect("root");
        for uri in [
            "../secret.rs",
            "%2e%2e/secret.rs",
            "https://example.com/a",
            "bad%QQ",
            "a%00.rs",
        ] {
            assert!(
                import_bytes(
                    &serde_json::to_vec(&fixture(uri)).expect("json"),
                    root.path(),
                    Language::Rust
                )
                .is_err(),
                "{uri}"
            );
        }
        assert!(import_bytes(b"{}", root.path(), Language::Rust).is_err());
        assert!(
            import_bytes(
                &vec![b' '; MAX_SARIF_BYTES + 1],
                root.path(),
                Language::Rust
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn missing_cli_does_not_start_or_install_anything() {
        let root = tempfile::tempdir().expect("root");
        assert!(
            analyze_codeql(
                Path::new("missing-codeql"),
                root.path(),
                root.path(),
                root.path(),
                Language::Rust,
                1
            )
            .await
            .is_err()
        );
        assert_eq!(std::fs::read_dir(root.path()).expect("root").count(), 0);
    }

    #[tokio::test]
    async fn explicit_adapter_accepts_only_successful_valid_evidence() {
        let root = tempfile::tempdir().expect("root");
        let source = root.path().join("fake.rs");
        let program = root
            .path()
            .join(format!("fake-codeql{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(
            &source,
            r#"
fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(&args[1..3], &["database", "analyze"]);
    assert!(args.iter().any(|a| a == "--threads=1"));
    assert!(args.iter().any(|a| a == "--ram=1024"));
    let output = args.iter().find_map(|a| a.strip_prefix("--output=")).expect("output");
    if std::path::Path::new("fail").exists() { std::process::exit(2); }
    let evidence = std::fs::read("input.sarif").expect("fixture");
    std::fs::write(output, evidence).expect("output");
}
"#,
        )
        .expect("helper source");
        let build = CommandSpec::new(
            "rustc",
            vec![
                source.display().to_string(),
                "-o".into(),
                program.display().to_string(),
            ],
            root.path(),
        )
        .timeout(Duration::from_secs(60));
        assert!(
            VerificationPipeline
                .run_command(build)
                .await
                .expect("compile helper")
                .success()
        );
        let input = root.path().join("input.sarif");
        std::fs::write(&input, serde_json::to_vec(&fixture("a.rs")).expect("json"))
            .expect("fixture");
        let analyze = || {
            analyze_codeql(
                &program,
                root.path(),
                root.path(),
                root.path(),
                Language::Rust,
                10,
            )
        };
        assert_eq!(analyze().await.expect("analysis").diagnostics.len(), 1);
        std::fs::write(root.path().join("fail"), "").expect("failure marker");
        assert!(analyze().await.is_err());
        std::fs::remove_file(root.path().join("fail")).expect("remove marker");
        std::fs::write(&input, "malformed").expect("invalid fixture");
        assert!(analyze().await.is_err());
    }
}
