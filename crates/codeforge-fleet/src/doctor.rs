use std::path::{Path, PathBuf};
use std::time::Duration;

use codeforge_protocol::{ToolAdapterDescriptor, ToolGap, ToolchainEntry, ToolchainSnapshot};
use codeforge_verification::{CommandSpec, VerificationPipeline};

use crate::tools::ToolRegistry;

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub entries: Vec<ToolchainEntry>,
    pub gaps: Vec<ToolGap>,
    pub snapshot: ToolchainSnapshot,
}

#[derive(Debug, Clone, Default)]
pub struct Doctor;

impl Doctor {
    pub async fn scan(root: impl AsRef<Path>) -> DoctorReport {
        let root = root.as_ref();
        let registry = ToolRegistry::with_all_builtins(root);
        let pipeline = VerificationPipeline;
        let mut entries = Vec::new();
        let mut gaps = Vec::new();
        for descriptor in registry.descriptors() {
            let required_for = required_for(&descriptor);
            let mut available = descriptor.available;
            let mut reason = descriptor.reason.clone();
            let mut version = match descriptor.executable.as_deref() {
                Some(executable) => version_of(&pipeline, executable).await,
                None => None,
            };
            if descriptor.id == "psscriptanalyzer"
                && let Some(executable) = descriptor.executable.as_deref()
            {
                let module = psscriptanalyzer_module(&pipeline, executable).await;
                available = module.is_some();
                reason = (!available)
                    .then(|| "PSScriptAnalyzer module was not found in pwsh".to_owned());
                version = module;
            }
            let normalized_path = descriptor
                .executable
                .as_deref()
                .map(|path| normalize_executable_path(root, path));
            let status = if available {
                "ok".to_owned()
            } else {
                "missing".to_owned()
            };
            let install_hint = (!available).then(|| install_hint(&descriptor.id));
            if !available {
                gaps.push(ToolGap {
                    tool: descriptor.id.clone(),
                    required_for: required_for.clone(),
                    status: "missing".to_owned(),
                    reason,
                    install_hint: install_hint.clone(),
                    repository: None,
                });
            }
            entries.push(ToolchainEntry {
                tool: descriptor.id.clone(),
                required_for,
                detected: available,
                path: normalized_path,
                version,
                capabilities: descriptor.capabilities.clone(),
                status,
                install_hint,
            });
        }
        let snapshot = ToolchainSnapshot {
            captured_at: chrono::Utc::now(),
            entries: entries.clone(),
        };
        DoctorReport {
            root: root.to_path_buf(),
            entries,
            gaps,
            snapshot,
        }
    }
}

async fn psscriptanalyzer_module(
    pipeline: &VerificationPipeline,
    executable: &Path,
) -> Option<String> {
    let result = pipeline
        .run_command(
            CommandSpec::new(
                executable,
                [
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Get-Module -ListAvailable PSScriptAnalyzer | Select-Object -First 1 | ConvertTo-Json -Compress",
                ],
                executable.parent().unwrap_or(Path::new(".")),
            )
            .timeout(Duration::from_secs(10)),
        )
        .await
        .ok()?;
    if !result.success() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(result.stdout.trim()).ok()?;
    value
        .get("Version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or_else(|| Some("detected".to_owned()))
}

async fn version_of(pipeline: &VerificationPipeline, executable: &Path) -> Option<String> {
    let result = pipeline
        .run_command(
            CommandSpec::new(
                executable,
                ["--version"],
                executable.parent().unwrap_or(Path::new(".")),
            )
            .timeout(Duration::from_secs(5)),
        )
        .await
        .ok()?;
    result.success().then(|| {
        result
            .stdout
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned()
    })
}

fn normalize_executable_path(root: &Path, path: &Path) -> PathBuf {
    if let Ok(relative) = path.strip_prefix(root) {
        relative.to_path_buf()
    } else {
        path.file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| path.to_path_buf())
    }
}

fn required_for(descriptor: &ToolAdapterDescriptor) -> String {
    let languages = descriptor
        .languages
        .iter()
        .map(|language| language.display_name())
        .collect::<Vec<_>>()
        .join(", ");
    let mut capabilities = Vec::new();
    if descriptor.capabilities.format {
        capabilities.push("format");
    }
    if descriptor.capabilities.lint {
        capabilities.push("lint");
    }
    if descriptor.capabilities.verify {
        capabilities.push("verify");
    }
    if descriptor.capabilities.benchmark {
        capabilities.push("benchmark");
    }
    format!("{languages} {}", capabilities.join("/"))
}

fn install_hint(id: &str) -> String {
    match id {
        "dart-format" | "dart-analyze" => {
            "Install the official Dart SDK, then rerun `codeforge doctor`.".to_owned()
        }
        "flutter-analyze" => {
            "Install the official Flutter SDK, then rerun `codeforge doctor`.".to_owned()
        }
        "psscriptanalyzer" => {
            "Install the PSScriptAnalyzer PowerShell module, then rerun `codeforge doctor`."
                .to_owned()
        }
        "clippy" => {
            "Install the Rust clippy component with rustup, then rerun `codeforge doctor`."
                .to_owned()
        }
        "ruff-format" | "ruff-check" => {
            "Install Ruff using the official Python packaging workflow, then rerun `codeforge doctor`."
                .to_owned()
        }
        "biome" | "prettier" | "eslint" | "oxlint" | "markdownlint" => {
            "Install the project-approved Node tool locally, then rerun `codeforge doctor`."
                .to_owned()
        }
        "clang-format" | "clang-tidy" => {
            "Install the project-approved LLVM toolchain, then rerun `codeforge doctor`."
                .to_owned()
        }
        "gofmt" | "gofumpt" | "staticcheck" => {
            "Install the project-approved Go tooling, then rerun `codeforge doctor`.".to_owned()
        }
        _ => "Install the required local tool, then rerun `codeforge doctor`.".to_owned(),
    }
}
