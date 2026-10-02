use std::path::PathBuf;

use codeforge_core::{CodeForgeEngine, ReviewOptions};
use codeforge_protocol::Language;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures")
        .join(name)
}

async fn assert_language(language: Language, directory: &str, bad_fragment: &str) {
    let engine = CodeForgeEngine::open(fixture(directory)).expect("open fixture");
    let report = engine
        .review(ReviewOptions {
            languages: vec![language],
            changed_only: false,
            include_external: false,
        })
        .await
        .expect("review");

    assert_eq!(report.files_analyzed, 4, "{directory}");
    assert!(report.languages.contains(&language));
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == "PARSE-001"
                && diagnostic.file.to_string_lossy().contains(bad_fragment)
        }),
        "bad fixture in {directory} did not produce a parse error"
    );
    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .file
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("fixable")
                && !diagnostic.fixes.is_empty()
        }),
        "fixable fixture in {directory} did not produce a fix"
    );
}

#[tokio::test]
async fn python_fixture_suite() {
    assert_language(Language::Python, "python", "bad.py").await;
}

#[tokio::test]
async fn rust_fixture_suite() {
    assert_language(Language::Rust, "rust", "bad.rs").await;
}

#[tokio::test]
async fn c_fixture_suite() {
    assert_language(Language::C, "c", "bad.c").await;
}

#[tokio::test]
async fn javascript_fixture_suite() {
    assert_language(Language::JavaScript, "javascript", "bad.js").await;
}

#[tokio::test]
async fn java_fixture_suite() {
    assert_language(Language::Java, "java", "Bad.java").await;
}

#[tokio::test]
async fn go_fixture_suite() {
    assert_language(Language::Go, "go", "bad.go").await;
}

#[tokio::test]
async fn dart_fixture_suite() {
    let engine = CodeForgeEngine::open(fixture("dart")).expect("open fixture");
    let report = engine
        .review(ReviewOptions {
            languages: vec![Language::Dart],
            changed_only: false,
            include_external: false,
        })
        .await
        .expect("review");
    assert_eq!(report.files_analyzed, 5);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule_id == "PARSE-001" && diagnostic.file.to_string_lossy().contains("bad.dart")
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule_id.starts_with("CF-DART-")
            && diagnostic.file.to_string_lossy().contains("fixable.dart")
    }));
}

#[tokio::test]
async fn powershell_fixture_suite() {
    let engine = CodeForgeEngine::open(fixture("powershell")).expect("open fixture");
    let report = engine
        .review(ReviewOptions {
            languages: vec![Language::PowerShell],
            changed_only: false,
            include_external: false,
        })
        .await
        .expect("review");
    assert_eq!(report.files_analyzed, 5);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule_id == "PARSE-001" && diagnostic.file.to_string_lossy().contains("bad.ps1")
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule_id.starts_with("CF-PS-")
            && diagnostic.file.to_string_lossy().contains("refactor.ps1")
    }));
}

#[tokio::test]
async fn markdown_fixture_suite() {
    let engine = CodeForgeEngine::open(fixture("markdown")).expect("open fixture");
    let report = engine
        .review(ReviewOptions {
            languages: vec![Language::Markdown],
            changed_only: false,
            include_external: false,
        })
        .await
        .expect("review");
    assert_eq!(report.files_analyzed, 5);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.rule_id == "CF-MD-001" && diagnostic.file.to_string_lossy().contains("bad.md")
    }));
}
