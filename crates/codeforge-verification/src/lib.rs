//! Structured process execution and verification pipelines.
//!
//! Commands are always represented as executable plus argument arrays. The crate
//! never invokes a shell, and backends are isolated by process boundaries.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use codeforge_protocol::{VerificationCheck, VerificationResult, VerificationStatus};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("failed to start verification command {program}: {source}")]
    Start {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub env: BTreeMap<String, String>,
    pub label: String,
}

impl CommandSpec {
    pub fn new(
        program: impl Into<PathBuf>,
        args: impl IntoIterator<Item = impl Into<String>>,
        cwd: impl Into<PathBuf>,
    ) -> Self {
        let program = program.into().to_string_lossy().into_owned();
        Self {
            label: program.clone(),
            program,
            args: args.into_iter().map(Into::into).collect(),
            cwd: cwd.into(),
            timeout: Duration::from_secs(300),
            max_output_bytes: 2 * 1024 * 1024,
            env: BTreeMap::new(),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_output_bytes(mut self, limit: usize) -> Self {
        self.max_output_bytes = limit.max(1);
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn command_line(&self) -> Vec<String> {
        std::iter::once(self.program.clone())
            .chain(self.args.iter().cloned())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
    pub timed_out: bool,
    pub output_truncated: bool,
}

impl ProcessResult {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0) && !self.timed_out
    }
}

#[derive(Debug, Clone, Default)]
pub struct VerificationPipeline;

#[derive(Debug, Clone, Default)]
pub struct VerificationPlan {
    pub syntax: Option<CommandSpec>,
    pub typecheck: Option<CommandSpec>,
    pub build: Option<CommandSpec>,
    pub tests: Option<CommandSpec>,
    pub fuzz: Option<CommandSpec>,
    pub differential: Option<CommandSpec>,
    pub equivalence: Option<CommandSpec>,
    pub benchmark: Option<CommandSpec>,
}

impl VerificationPipeline {
    pub async fn run(&self, plan: VerificationPlan) -> VerificationResult {
        let syntax = self.run_optional(plan.syntax).await;
        let typecheck = self.run_optional(plan.typecheck).await;
        let build = self.run_optional(plan.build).await;
        let tests = self.run_optional(plan.tests).await;
        let fuzz = self.run_optional(plan.fuzz).await;
        let differential = self.run_optional(plan.differential).await;
        let equivalence = self.run_optional(plan.equivalence).await;
        let benchmark = self.run_optional(plan.benchmark).await;
        VerificationResult {
            syntax,
            typecheck,
            build,
            tests,
            fuzz,
            differential,
            equivalence,
            benchmark,
        }
    }

    pub async fn run_optional(&self, command: Option<CommandSpec>) -> VerificationCheck {
        match command {
            None => VerificationCheck {
                status: VerificationStatus::Skipped,
                message: Some("no verifier configured".to_owned()),
                duration_ms: None,
                command: None,
            },
            Some(command) => {
                let command_line = command.command_line();
                match self.run_command(command).await {
                    Ok(result) if result.success() => {
                        let summary = summarize_process(&result);
                        VerificationCheck {
                            status: VerificationStatus::Passed,
                            message: Some(summary),
                            duration_ms: Some(result.elapsed.as_millis()),
                            command: Some(command_line),
                        }
                    }
                    Ok(result) => {
                        let summary = summarize_process(&result);
                        VerificationCheck {
                            status: VerificationStatus::Failed,
                            message: Some(summary),
                            duration_ms: Some(result.elapsed.as_millis()),
                            command: Some(command_line),
                        }
                    }
                    Err(error) => VerificationCheck {
                        status: VerificationStatus::Unavailable,
                        message: Some(error.to_string()),
                        duration_ms: None,
                        command: Some(command_line),
                    },
                }
            }
        }
    }

    pub async fn run_command(&self, spec: CommandSpec) -> Result<ProcessResult, VerificationError> {
        let started = Instant::now();
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let mut child = command.spawn().map_err(|source| VerificationError::Start {
            program: spec.program.clone(),
            source,
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            VerificationError::Io(std::io::Error::other("child stdout was not captured"))
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            VerificationError::Io(std::io::Error::other("child stderr was not captured"))
        })?;
        let output_limit = spec.max_output_bytes;
        let stdout_task = tokio::spawn(read_limited(stdout, output_limit));
        let stderr_task = tokio::spawn(read_limited(stderr, output_limit));

        let deadline = Instant::now() + spec.timeout;
        let (status, timed_out) = loop {
            if let Some(status) = child.try_wait()? {
                break (status, false);
            }
            if Instant::now() >= deadline {
                child.kill().await?;
                break (child.wait().await?, true);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        let (stdout, stdout_truncated) = stdout_task
            .await
            .map_err(|error| VerificationError::Io(std::io::Error::other(error)))??;
        let (stderr, stderr_truncated) = stderr_task
            .await
            .map_err(|error| VerificationError::Io(std::io::Error::other(error)))??;

        Ok(ProcessResult {
            exit_code: status.code(),
            stdout,
            stderr,
            elapsed: started.elapsed(),
            timed_out,
            output_truncated: stdout_truncated || stderr_truncated,
        })
    }
}

async fn read_limited<R>(mut reader: R, limit: usize) -> std::io::Result<(String, bool)>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(output.len());
        if remaining > 0 {
            output.extend_from_slice(&buffer[..count.min(remaining)]);
        }
        if count > remaining {
            truncated = true;
        }
    }
    Ok((String::from_utf8_lossy(&output).into_owned(), truncated))
}

fn summarize_process(result: &ProcessResult) -> String {
    let mut summary = if result.timed_out {
        "timed out".to_owned()
    } else {
        format!("exit code {}", result.exit_code.unwrap_or(-1))
    };
    let details = if result.stderr.trim().is_empty() {
        result.stdout.trim()
    } else {
        result.stderr.trim()
    };
    if !details.is_empty() {
        summary.push_str(": ");
        summary.extend(details.chars().take(500));
    }
    if result.output_truncated {
        summary.push_str(" [output truncated]");
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn captures_successful_process() {
        let spec = CommandSpec::new(
            "rustc",
            ["--version"],
            std::env::current_dir().expect("cwd"),
        )
        .timeout(Duration::from_secs(10));
        let result = VerificationPipeline.run_command(spec).await.expect("run");
        assert!(result.success());
        assert!(result.stdout.contains("rustc"));
    }

    #[tokio::test]
    async fn reports_missing_executable_as_unavailable() {
        let spec = CommandSpec::new(
            "codeforge-executable-that-does-not-exist",
            std::iter::empty::<&str>(),
            std::env::current_dir().expect("cwd"),
        );
        let result = VerificationPipeline.run_optional(Some(spec)).await;
        assert_eq!(result.status, VerificationStatus::Unavailable);
    }
}
