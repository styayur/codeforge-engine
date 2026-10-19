use std::fs;
use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeforge"))
        .args(arguments)
        .output()
        .expect("run codeforge")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("utf-8 stdout")
}

#[test]
fn apply_history_and_undo_round_trip() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    let source = workspace.path().join("fixable.rs");
    fs::write(
        &source,
        "pub fn empty(values: &[i32]) -> bool {\n    values.len() == 0\n}\n",
    )
    .expect("write fixture");
    let root = workspace.path().to_string_lossy().into_owned();

    let preview = run(&[
        "apply",
        &root,
        "--rule",
        "RS-PERFORMANCE-001",
        "--file",
        "fixable.rs",
        "--json",
    ]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert!(stdout(&preview).contains("\"applied\": false"));
    assert!(
        fs::read_to_string(&source)
            .expect("read test fixture")
            .contains("values.len() == 0")
    );

    let apply = run(&[
        "apply",
        &root,
        "--rule",
        "RS-PERFORMANCE-001",
        "--file",
        "fixable.rs",
        "--apply",
        "--allow-unsafe",
        "--json",
    ]);
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
    let applied: serde_json::Value = serde_json::from_str(&stdout(&apply)).expect("apply json");
    let transaction_id = applied["transaction"]["id"]
        .as_str()
        .expect("transaction id")
        .to_owned();
    assert_eq!(applied["applied"], true);
    assert!(
        fs::read_to_string(&source)
            .expect("read test fixture")
            .contains("values.is_empty()")
    );

    let history = run(&["history", &root, "--json"]);
    assert!(
        history.status.success(),
        "{}",
        String::from_utf8_lossy(&history.stderr)
    );
    let records: serde_json::Value = serde_json::from_str(&stdout(&history)).expect("history json");
    assert_eq!(records[0]["id"], transaction_id);

    let undo = run(&["undo", &root, "--transaction", &transaction_id, "--json"]);
    assert!(
        undo.status.success(),
        "{}",
        String::from_utf8_lossy(&undo.stderr)
    );
    assert!(
        fs::read_to_string(&source)
            .expect("read test fixture")
            .contains("values.len() == 0")
    );

    let history = run(&["history", &root, "--json"]);
    assert!(history.status.success());
    assert_eq!(stdout(&history).trim(), "[]");
}

#[test]
fn ambiguous_apply_selector_is_refused() {
    let workspace = tempfile::tempdir().expect("temporary workspace");
    fs::write(
        workspace.path().join("one.rs"),
        "pub fn one(values: &[i32]) -> bool { values.len() == 0 }\n",
    )
    .expect("test fixture operation");
    fs::write(
        workspace.path().join("two.rs"),
        "pub fn two(values: &[i32]) -> bool { values.len() == 0 }\n",
    )
    .expect("test fixture operation");
    let root = workspace.path().to_string_lossy().into_owned();

    let output = run(&[
        "apply",
        &root,
        "--rule",
        "RS-PERFORMANCE-001",
        "--apply",
        "--allow-unsafe",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ambiguous fix selector"));
}
