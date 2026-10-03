use std::path::Path;

use codeforge_engines::analyze_source;
use codeforge_protocol::{Confidence, Language, Severity, SourceContext};

fn fixture(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/regressions")
        .join(path)
}

#[test]
fn test_context_downgrades_rust_unwrap_findings() {
    let path = fixture("rs-correctness-test-unwrap/test.rs");
    let source = std::fs::read_to_string(&path).expect("fixture");
    let findings = analyze_source("test", Language::Rust, &path, &source);
    let finding = findings
        .iter()
        .find(|finding| finding.rule_id == "RS-CORRECTNESS-001")
        .expect("unwrap finding");
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(finding.confidence, Confidence::Low);
    assert_eq!(finding.source_context, SourceContext::Fixture);
}

#[test]
fn safety_comment_changes_unsafe_finding_confidence() {
    let safe_path = fixture("rs-security-win32-ffi/safe.rs");
    let safe = std::fs::read_to_string(&safe_path).expect("safe fixture");
    let safe_finding = analyze_source("test", Language::Rust, &safe_path, &safe)
        .into_iter()
        .find(|finding| finding.rule_id == "RS-SECURITY-001")
        .expect("safe unsafe finding");
    assert_eq!(safe_finding.severity, Severity::Info);
    assert_eq!(safe_finding.confidence, Confidence::Low);

    let unsafe_path = fixture("rs-security-win32-ffi/unsafe.rs");
    let unsafe_source = std::fs::read_to_string(&unsafe_path).expect("unsafe fixture");
    let unsafe_finding = analyze_source("test", Language::Rust, &unsafe_path, &unsafe_source)
        .into_iter()
        .find(|finding| finding.rule_id == "RS-SECURITY-001")
        .expect("unsafe finding");
    assert_eq!(unsafe_finding.severity, Severity::Warning);
    assert_eq!(unsafe_finding.confidence, Confidence::Medium);
}

#[test]
fn toml_rule_accepts_custom_schemas_and_rejects_true_duplicates() {
    for name in ["codeforge.toml", "cargo.toml", "generic.toml"] {
        let path = fixture(&format!("cf-toml-custom-config/{name}"));
        let source = std::fs::read_to_string(&path).expect("valid fixture");
        let findings = analyze_source("test", Language::Toml, &path, &source);
        assert!(findings.is_empty(), "{name} should be valid TOML");
    }

    let path = fixture("cf-toml-custom-config/invalid.toml");
    let source = std::fs::read_to_string(&path).expect("invalid fixture");
    let findings = analyze_source("test", Language::Toml, &path, &source);
    assert!(
        findings
            .iter()
            .any(|finding| finding.rule_id == "CF-TOML-001")
    );
}
