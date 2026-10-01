//! Optional AI provider boundary. AI is disabled by default and cannot bypass verification.

use async_trait::async_trait;
use codeforge_protocol::{Diagnostic, Transformation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiPolicy {
    pub enabled: bool,
    pub source_upload_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExplanation {
    pub text: String,
    #[serde(default)]
    pub citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTransformProposal {
    pub explanation: String,
    pub transformations: Vec<Transformation>,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("AI integration is disabled")]
    Disabled,
    #[error("the selected provider does not support this operation")]
    Unsupported,
    #[error("AI provider failed: {0}")]
    Provider(String),
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn policy(&self) -> &AiPolicy;

    async fn explain_diagnostic(&self, _diagnostic: &Diagnostic) -> Result<AiExplanation, AiError> {
        Err(AiError::Unsupported)
    }

    async fn propose_transformations(
        &self,
        _diagnostic: &Diagnostic,
    ) -> Result<AiTransformProposal, AiError> {
        Err(AiError::Unsupported)
    }
}

#[derive(Default)]
pub struct DisabledAiProvider {
    policy: AiPolicy,
}

#[async_trait]
impl AiProvider for DisabledAiProvider {
    fn policy(&self) -> &AiPolicy {
        &self.policy
    }

    async fn explain_diagnostic(&self, _diagnostic: &Diagnostic) -> Result<AiExplanation, AiError> {
        Err(AiError::Disabled)
    }

    async fn propose_transformations(
        &self,
        _diagnostic: &Diagnostic,
    ) -> Result<AiTransformProposal, AiError> {
        Err(AiError::Disabled)
    }
}
