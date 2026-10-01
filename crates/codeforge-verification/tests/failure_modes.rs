use std::time::Duration;

use codeforge_protocol::VerificationStatus;
use codeforge_verification::{CommandSpec, VerificationPipeline};

#[tokio::test]
async fn missing_executable_is_unavailable() {
    let spec = CommandSpec::new(
        "codeforge-command-does-not-exist",
        std::iter::empty::<&str>(),
        std::env::current_dir().expect("cwd"),
    );
    let result = VerificationPipeline.run_optional(Some(spec)).await;
    assert_eq!(result.status, VerificationStatus::Unavailable);
}

#[tokio::test]
async fn timeout_is_reported_and_process_is_stopped() {
    let cwd = std::env::current_dir().expect("cwd");
    #[cfg(windows)]
    let spec = CommandSpec::new(
        "powershell",
        ["-NoProfile", "-Command", "Start-Sleep -Seconds 5"],
        cwd,
    )
    .timeout(Duration::from_millis(100));
    #[cfg(unix)]
    let spec = CommandSpec::new("sh", ["-c", "sleep 5"], cwd).timeout(Duration::from_millis(100));

    let result = VerificationPipeline.run_command(spec).await.expect("run");
    assert!(result.timed_out);
    assert!(!result.success());
}

#[tokio::test]
async fn compile_failure_is_failed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("broken.rs");
    std::fs::write(&source, "fn main( {\n").expect("write");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let spec = CommandSpec::new(rustc, [source.to_string_lossy().to_string()], temp.path())
        .timeout(Duration::from_secs(30));
    let result = VerificationPipeline.run_optional(Some(spec)).await;
    assert_eq!(result.status, VerificationStatus::Failed);
}
