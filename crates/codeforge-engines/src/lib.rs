//! Language adapters and engine registry.
//!
//! Built-in adapters use each language's native Tree-sitter grammar. External
//! tools stay behind explicit process boundaries and are never hard dependencies.

mod builtin;
mod language;
mod tools;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use codeforge_protocol::{
    BenchmarkResult, Capability, Diagnostic, EngineMetadata, EngineStatus, Language,
    Transformation, VerificationResult,
};
pub use tools::{
    DiscoveredTool, ToolDefinition, discover_tools, find_executable, find_executable_in_workspace,
    tool_definitions,
};

pub fn analyze_source(
    engine: &str,
    language: Language,
    path: &Path,
    source: &str,
) -> Vec<Diagnostic> {
    builtin::analyze_source(engine, language, path, source)
}

pub fn transformations_from_source(
    engine: &str,
    language: Language,
    path: &Path,
    source: &str,
) -> Vec<Transformation> {
    builtin::transformations_from_source(engine, language, path, source)
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("engine does not support {capability}: {engine}")]
    Unsupported {
        engine: String,
        capability: &'static str,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("engine failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct EngineRequest {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
}

impl EngineRequest {
    pub fn new(root: impl Into<PathBuf>, files: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            root: root.into(),
            files: files.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EngineOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub transformations: Vec<Transformation>,
}

impl EngineOutput {
    pub fn extend(&mut self, other: Self) {
        self.diagnostics.extend(other.diagnostics);
        self.transformations.extend(other.transformations);
    }
}

#[async_trait]
pub trait AnalysisEngine: Send + Sync {
    fn metadata(&self) -> &EngineMetadata;
    fn status(&self) -> EngineStatus {
        EngineStatus {
            metadata: self.metadata().clone(),
            available: true,
            reason: None,
        }
    }

    async fn analyze(&self, _request: &EngineRequest) -> Result<EngineOutput, EngineError> {
        Err(EngineError::Unsupported {
            engine: self.metadata().id.clone(),
            capability: "analyze",
        })
    }

    async fn transform(
        &self,
        _request: &EngineRequest,
    ) -> Result<Vec<Transformation>, EngineError> {
        Err(EngineError::Unsupported {
            engine: self.metadata().id.clone(),
            capability: "transform",
        })
    }

    async fn verify(&self, _root: &Path) -> Result<VerificationResult, EngineError> {
        Err(EngineError::Unsupported {
            engine: self.metadata().id.clone(),
            capability: "verify",
        })
    }

    async fn benchmark(&self, _root: &Path) -> Result<Vec<BenchmarkResult>, EngineError> {
        Err(EngineError::Unsupported {
            engine: self.metadata().id.clone(),
            capability: "benchmark",
        })
    }
}

pub struct BuiltinAdapter {
    language: Language,
    metadata: EngineMetadata,
}

impl BuiltinAdapter {
    pub fn new(language: Language) -> Self {
        let id = format!("{}-builtin", language.as_str());
        Self {
            language,
            metadata: EngineMetadata {
                id,
                name: format!("{} Tree-sitter Adapter", language.display_name()),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                languages: vec![language],
                capabilities: vec![
                    Capability::Parse,
                    Capability::Lint,
                    Capability::Fix,
                    Capability::Refactor,
                    Capability::Optimize,
                ],
                executable: None,
                dependencies: Vec::new(),
                timeout_ms: Some(100),
                permissions: Vec::new(),
            },
        }
    }

    pub fn language(&self) -> Language {
        self.language
    }

    fn matching_files(&self, request: &EngineRequest) -> Vec<PathBuf> {
        request
            .files
            .iter()
            .map(|file| {
                if file.is_absolute() {
                    file.clone()
                } else {
                    request.root.join(file)
                }
            })
            .filter(|file| {
                file.extension()
                    .and_then(|extension| extension.to_str())
                    .and_then(Language::from_extension)
                    == Some(self.language)
            })
            .collect()
    }
}

#[async_trait]
impl AnalysisEngine for BuiltinAdapter {
    fn metadata(&self) -> &EngineMetadata {
        &self.metadata
    }

    async fn analyze(&self, request: &EngineRequest) -> Result<EngineOutput, EngineError> {
        let mut output = EngineOutput::default();
        for file in self.matching_files(request) {
            let source = match std::fs::read_to_string(&file) {
                Ok(source) => source,
                Err(error) => {
                    tracing::warn!(%error, path = %file.display(), "skipping unreadable analysis target");
                    continue;
                }
            };
            output.diagnostics.extend(builtin::analyze_source(
                &self.metadata.id,
                self.language,
                &file,
                &source,
            ));
        }
        Ok(output)
    }

    async fn transform(&self, request: &EngineRequest) -> Result<Vec<Transformation>, EngineError> {
        let mut transformations = Vec::new();
        for file in self.matching_files(request) {
            let source = match std::fs::read_to_string(&file) {
                Ok(source) => source,
                Err(error) => {
                    tracing::warn!(%error, path = %file.display(), "skipping unreadable transform target");
                    continue;
                }
            };
            transformations.extend(builtin::transformations_from_source(
                &self.metadata.id,
                self.language,
                &file,
                &source,
            ));
        }
        Ok(transformations)
    }
}

pub struct ExternalToolAdapter {
    metadata: EngineMetadata,
    available: bool,
    reason: Option<String>,
}

impl ExternalToolAdapter {
    pub fn from_discovered(tool: DiscoveredTool) -> Self {
        let status = tool.status();
        Self {
            metadata: status.metadata,
            available: status.available,
            reason: status.reason,
        }
    }
}

#[async_trait]
impl AnalysisEngine for ExternalToolAdapter {
    fn metadata(&self) -> &EngineMetadata {
        &self.metadata
    }

    fn status(&self) -> EngineStatus {
        EngineStatus {
            metadata: self.metadata.clone(),
            available: self.available,
            reason: self.reason.clone(),
        }
    }
}

#[derive(Default)]
pub struct EngineRegistry {
    engines: Vec<Arc<dyn AnalysisEngine>>,
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtin_adapters() -> Self {
        let mut registry = Self::new();
        for language in Language::ALL {
            registry.register(Arc::new(BuiltinAdapter::new(language)));
        }
        registry
    }

    pub fn with_external_tools(workspace_root: &Path) -> Self {
        let mut registry = Self::new();
        for tool in discover_tools(workspace_root) {
            registry.register(Arc::new(ExternalToolAdapter::from_discovered(tool)));
        }
        registry
    }

    pub fn register(&mut self, engine: Arc<dyn AnalysisEngine>) {
        self.engines.push(engine);
    }

    pub fn extend(&mut self, other: Self) {
        self.engines.extend(other.engines);
    }

    pub fn engines(&self) -> &[Arc<dyn AnalysisEngine>] {
        &self.engines
    }

    pub fn metadata(&self) -> Vec<EngineMetadata> {
        self.engines
            .iter()
            .map(|engine| engine.metadata().clone())
            .collect()
    }

    pub fn statuses(&self) -> Vec<EngineStatus> {
        self.engines.iter().map(|engine| engine.status()).collect()
    }

    pub fn for_language(&self, language: Language) -> Vec<Arc<dyn AnalysisEngine>> {
        self.engines
            .iter()
            .filter(|engine| engine.metadata().languages.contains(&language))
            .cloned()
            .collect()
    }

    pub async fn analyze(&self, request: &EngineRequest) -> EngineOutput {
        let mut output = EngineOutput::default();
        for engine in &self.engines {
            match engine.analyze(request).await {
                Ok(engine_output) => output.extend(engine_output),
                Err(EngineError::Unsupported { .. }) => {}
                Err(error) => {
                    tracing::warn!(engine = %engine.metadata().id, %error, "analysis engine failed");
                }
            }
        }
        output
    }

    pub async fn transform(&self, request: &EngineRequest) -> Vec<Transformation> {
        let mut transformations = Vec::new();
        for engine in &self.engines {
            match engine.transform(request).await {
                Ok(mut engine_transformations) => {
                    transformations.append(&mut engine_transformations)
                }
                Err(EngineError::Unsupported { .. }) => {}
                Err(error) => {
                    tracing::warn!(engine = %engine.metadata().id, %error, "transform engine failed");
                }
            }
        }
        transformations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builtin_adapters_cover_all_languages() {
        let registry = EngineRegistry::with_builtin_adapters();
        for language in Language::ALL {
            assert!(!registry.for_language(language).is_empty());
        }
    }
}
