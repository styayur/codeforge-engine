use std::fs;

use codeforge_core::{CodeForgeEngine, ReviewOptions};

#[tokio::test]
async fn safe_ast_fix_is_idempotent_and_parseable() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("example.rs");
    fs::write(
        &path,
        "fn check(value: &str) -> bool {\n    value.len() == 0\n}\n",
    )
    .expect("fixture");
    let engine = CodeForgeEngine::open(temp.path()).expect("engine");
    let first = engine
        .review(ReviewOptions::default())
        .await
        .expect("first review");
    let diagnostic = first
        .diagnostics
        .iter()
        .find(|diagnostic| !diagnostic.fixes.is_empty())
        .expect("safe fix");
    let preview = engine
        .preview_fix(&diagnostic.id, 0)
        .expect("preview")
        .preview;
    engine
        .apply_preview(&preview, "idempotency fixture", false)
        .expect("apply");
    assert_eq!(
        fs::read_to_string(&path).expect("read"),
        "fn check(value: &str) -> bool {\n    value.is_empty()\n}\n"
    );

    let second = engine
        .review(ReviewOptions::default())
        .await
        .expect("second review");
    assert!(
        !second
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.rule_id == "RUST-001" && !diagnostic.fixes.is_empty() })
    );
}
