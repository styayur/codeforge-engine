//! Repository fleet orchestration, policy enforcement, and evidence generation.

mod cache;
mod config;
mod detect;
mod doctor;
mod evidence;
mod pr;
mod runner;
mod tools;

use std::path::PathBuf;

pub use cache::{CacheKey, FleetCache};
pub use config::{
    FleetConfig, FleetPolicy, FleetSection, ProtectedPaths, ResolvedRepository, is_generated,
};
pub use detect::{DetectedWorkspace, ProjectDetector, suggest_codeforge_toml};
pub use doctor::{Doctor, DoctorReport};
pub use evidence::{EvidenceInput, EvidenceSnapshot, EvidenceWriter, WrittenEvidence};
pub use pr::{PrManager, PrMode, PrOutcome};
pub use runner::{
    DefaultRepoExecutor, FleetCommand, FleetRunOptions, FleetRunner, RepoExecutionContext,
    RepoOperationExecutor,
};
pub use tools::{ToolAdapter, ToolContext, ToolOperation, ToolPlan, ToolRegistry, ToolRunner};

#[derive(Debug, thiserror::Error)]
pub enum FleetError {
    #[error("cannot read fleet configuration {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse fleet configuration {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid fleet configuration: {0}")]
    InvalidConfig(String),
    #[error("repository is missing or not a directory: {0}")]
    MissingRepository(PathBuf),
    #[error("repository escapes the configured fleet workspace: {0}")]
    RepositoryOutsideWorkspace(PathBuf),
    #[error("operation is unavailable for {repository}: {reason}")]
    ToolUnavailable { repository: String, reason: String },
    #[error("verification failed for {repository}: {reason}")]
    VerificationFailed { repository: String, reason: String },
    #[error("safety policy refused {repository}: {reason}")]
    SafetyRefusal { repository: String, reason: String },
    #[error("transformation failed for {repository}: {reason}")]
    TransformationFailed { repository: String, reason: String },
    #[error(transparent)]
    Core(#[from] codeforge_core::CoreError),
    #[error(transparent)]
    Transform(#[from] codeforge_transform::TransformError),
    #[error(transparent)]
    Sandbox(#[from] codeforge_core::SandboxError),
    #[error(transparent)]
    Verification(#[from] codeforge_verification::VerificationError),
    #[error(transparent)]
    Protocol(#[from] codeforge_protocol::ProtocolError),
    #[error(transparent)]
    Git(#[from] codeforge_git::GitError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("fleet worker task failed: {0}")]
    Join(String),
}
