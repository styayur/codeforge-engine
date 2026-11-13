//! Declarative plugin and engine manifests with explicit permissions.

use std::fs;
use std::path::{Path, PathBuf};

use codeforge_protocol::{Capability, EngineMetadata, Language, PluginPermission};
use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("invalid plugin id: {0}")]
    InvalidId(String),
    #[error("invalid plugin version {version}: {message}")]
    InvalidVersion { version: String, message: String },
    #[error("plugin {0} declares no language")]
    NoLanguages(String),
    #[error("plugin {0} declares unsupported permissions for its adapter kind")]
    UnsafePermissions(String),
    #[error("plugin file is missing: {0}")]
    Missing(PathBuf),
    #[error("cannot parse plugin manifest {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    Bundled,
    LocalExecutable,
    DynamicAdapter,
    ExternalWorker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: PluginKind,
    pub languages: Vec<Language>,
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub executable: Option<PathBuf>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub description: Option<String>,
}

impl PluginManifest {
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.id.is_empty()
            || !self.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
            })
        {
            return Err(PluginError::InvalidId(self.id.clone()));
        }
        Version::parse(&self.version).map_err(|error| PluginError::InvalidVersion {
            version: self.version.clone(),
            message: error.to_string(),
        })?;
        if self.languages.is_empty() {
            return Err(PluginError::NoLanguages(self.id.clone()));
        }
        if matches!(self.kind, PluginKind::Bundled | PluginKind::DynamicAdapter)
            && self.permissions.contains(&PluginPermission::Network)
        {
            return Err(PluginError::UnsafePermissions(self.id.clone()));
        }
        if matches!(self.kind, PluginKind::Bundled)
            && self
                .permissions
                .iter()
                .any(|permission| *permission != PluginPermission::ReadWorkspace)
        {
            return Err(PluginError::UnsafePermissions(self.id.clone()));
        }
        Ok(())
    }

    pub fn metadata(&self) -> EngineMetadata {
        EngineMetadata {
            id: self.id.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            languages: self.languages.clone(),
            capabilities: self.capabilities.clone(),
            executable: self.executable.clone(),
            dependencies: self.dependencies.clone(),
            timeout_ms: self.timeout_ms,
            permissions: self.permissions.clone(),
        }
    }
}

pub struct PluginManager {
    roots: Vec<PathBuf>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { roots: Vec::new() }
    }

    pub fn add_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.roots.push(root.into());
        self
    }

    pub fn discover(&self) -> Result<Vec<PluginManifest>, PluginError> {
        let mut manifests = Vec::new();
        for root in &self.roots {
            if !root.is_dir() {
                continue;
            }
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                let candidate = if entry.file_type()?.is_dir() {
                    entry.path().join("plugin.toml")
                } else if entry.path().file_name().and_then(|name| name.to_str())
                    == Some("plugin.toml")
                {
                    entry.path()
                } else {
                    continue;
                };
                if candidate.exists() {
                    manifests.push(load_manifest(&candidate)?);
                }
            }
        }
        manifests.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(manifests)
    }

    pub fn discover_engine_manifests(root: &Path) -> Result<Vec<PluginManifest>, PluginError> {
        let mut manifests = Vec::new();
        if !root.is_dir() {
            return Ok(manifests);
        }
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path().join("engine.toml");
            if path.exists() {
                manifests.push(load_manifest(&path)?);
            }
        }
        manifests.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(manifests)
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn load_manifest(path: impl AsRef<Path>) -> Result<PluginManifest, PluginError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(PluginError::Missing(path.to_path_buf()));
    }
    let content = fs::read_to_string(path)?;
    let manifest: PluginManifest =
        toml::from_str(&content).map_err(|source| PluginError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    manifest.validate()?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_declarative_engine_manifest() {
        let manifest = PluginManifest {
            id: "python-ruff".to_owned(),
            name: "Ruff".to_owned(),
            version: "0.2.1".to_owned(),
            kind: PluginKind::LocalExecutable,
            languages: vec![Language::Python],
            capabilities: vec![Capability::Lint, Capability::Fix],
            executable: Some(PathBuf::from("ruff")),
            dependencies: Vec::new(),
            timeout_ms: Some(30_000),
            permissions: vec![PluginPermission::ReadWorkspace],
            description: None,
        };
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn rejects_path_traversal_ids() {
        let manifest = PluginManifest {
            id: "../escape".to_owned(),
            name: "bad".to_owned(),
            version: "1.0.0".to_owned(),
            kind: PluginKind::ExternalWorker,
            languages: vec![Language::Python],
            capabilities: vec![Capability::Lint],
            executable: None,
            dependencies: Vec::new(),
            timeout_ms: None,
            permissions: Vec::new(),
            description: None,
        };
        assert!(manifest.validate().is_err());
    }
    #[test]
    fn bundled_engine_manifests_are_valid() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../engines");
        let manifests =
            PluginManager::discover_engine_manifests(&root).expect("discover manifests");
        assert_eq!(manifests.len(), 6);
        assert!(
            manifests
                .iter()
                .any(|manifest| manifest.id == "python-ruff")
        );
    }
}
