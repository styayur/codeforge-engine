use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

use crate::FleetError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetConfig {
    pub fleet: FleetSection,
    #[serde(default)]
    pub policy: FleetPolicy,
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
    #[serde(skip)]
    pub resolved_workspace_root: PathBuf,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            fleet: FleetSection::default(),
            policy: FleetPolicy::default(),
            config_path: None,
            resolved_workspace_root: PathBuf::from("."),
        }
    }
}

impl FleetConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, FleetError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|source| FleetError::ConfigRead {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&content, path)
    }

    pub fn from_toml_str(content: &str, source_path: impl AsRef<Path>) -> Result<Self, FleetError> {
        let source_path = source_path.as_ref();
        let mut config: Self =
            toml::from_str(content).map_err(|source| FleetError::ConfigParse {
                path: source_path.to_path_buf(),
                source,
            })?;
        if config.fleet.name.trim().is_empty() {
            return Err(FleetError::InvalidConfig(
                "fleet.name must not be empty".to_owned(),
            ));
        }
        if config.fleet.repositories.is_empty() {
            return Err(FleetError::InvalidConfig(
                "fleet.repositories must contain at least one repository".to_owned(),
            ));
        }
        if !config.policy.one_repository_per_transaction {
            return Err(FleetError::InvalidConfig(
                "one_repository_per_transaction must remain enabled".to_owned(),
            ));
        }
        if !config.policy.one_repository_per_pr {
            return Err(FleetError::InvalidConfig(
                "one_repository_per_pr must remain enabled".to_owned(),
            ));
        }
        if !config.policy.validate() {
            return Err(FleetError::InvalidConfig(
                "fleet policy contains invalid limits".to_owned(),
            ));
        }
        config.policy.protected_paths.compile()?;

        let base = source_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let workspace = if config.fleet.workspace_root.is_absolute() {
            config.fleet.workspace_root.clone()
        } else {
            base.join(&config.fleet.workspace_root)
        };
        config.resolved_workspace_root =
            workspace
                .canonicalize()
                .map_err(|source| FleetError::ConfigRead {
                    path: workspace,
                    source,
                })?;
        config.config_path = Some(source_path.to_path_buf());
        Ok(config)
    }

    pub fn repositories(&self) -> Result<Vec<ResolvedRepository>, FleetError> {
        let excluded = build_globset(&self.fleet.exclude, "fleet.exclude")?;
        let expected = self
            .fleet
            .repositories
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        let mut repositories = Vec::new();
        for configured in &self.fleet.repositories {
            if excluded.is_match(normalize(configured)) {
                continue;
            }
            let candidate = PathBuf::from(configured);
            let path = if candidate.is_absolute() {
                candidate
            } else {
                self.resolved_workspace_root.join(&candidate)
            };
            if !path.is_dir() {
                return Err(FleetError::MissingRepository(path));
            }
            let canonical = path.canonicalize()?;
            if !canonical.starts_with(&self.resolved_workspace_root) {
                return Err(FleetError::RepositoryOutsideWorkspace(canonical));
            }
            let name = canonical
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(configured)
                .to_owned();
            if expected.contains(name.as_str()) || expected.contains(configured.as_str()) {
                repositories.push(ResolvedRepository {
                    name,
                    path: canonical,
                });
            }
        }
        repositories.sort_by(|left, right| left.name.cmp(&right.name));
        if repositories.is_empty() {
            return Err(FleetError::InvalidConfig(
                "no repositories remain after exclusions".to_owned(),
            ));
        }
        Ok(repositories)
    }

    pub fn report_root(&self, override_dir: Option<&Path>) -> PathBuf {
        override_dir.map_or_else(
            || {
                self.resolved_workspace_root
                    .join(".codeforge")
                    .join("fleet")
            },
            Path::to_path_buf,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetSection {
    #[serde(default = "default_fleet_name")]
    pub name: String,
    #[serde(default = "default_workspace_root")]
    pub workspace_root: PathBuf,
    #[serde(default)]
    pub repositories: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub concurrency: Option<usize>,
}

impl Default for FleetSection {
    fn default() -> Self {
        Self {
            name: default_fleet_name(),
            workspace_root: default_workspace_root(),
            repositories: Vec::new(),
            exclude: Vec::new(),
            concurrency: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetPolicy {
    #[serde(default = "default_true")]
    pub require_clean_tree: bool,
    #[serde(default = "default_true")]
    pub one_repository_per_transaction: bool,
    #[serde(default = "default_true")]
    pub one_repository_per_pr: bool,
    #[serde(default = "default_true")]
    pub rollback_on_failure: bool,
    #[serde(default = "default_max_changed_files")]
    pub max_changed_files: usize,
    #[serde(default = "default_max_changed_lines")]
    pub max_changed_lines: usize,
    #[serde(default = "default_true")]
    pub require_tests: bool,
    #[serde(default = "default_true")]
    pub require_build: bool,
    #[serde(default = "default_branch_prefix")]
    pub branch_prefix: String,
    #[serde(default)]
    pub protected_paths: ProtectedPaths,
}

impl Default for FleetPolicy {
    fn default() -> Self {
        Self {
            require_clean_tree: true,
            one_repository_per_transaction: true,
            one_repository_per_pr: true,
            rollback_on_failure: true,
            max_changed_files: default_max_changed_files(),
            max_changed_lines: default_max_changed_lines(),
            require_tests: true,
            require_build: true,
            branch_prefix: default_branch_prefix(),
            protected_paths: ProtectedPaths::default(),
        }
    }
}

impl FleetPolicy {
    fn validate(&self) -> bool {
        self.max_changed_files > 0
            && self.max_changed_lines > 0
            && !self.branch_prefix.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedPaths {
    #[serde(default = "default_protected_patterns")]
    pub patterns: Vec<String>,
    #[serde(skip)]
    compiled: Option<GlobSet>,
}

impl Default for ProtectedPaths {
    fn default() -> Self {
        Self {
            patterns: default_protected_patterns(),
            compiled: None,
        }
    }
}

impl ProtectedPaths {
    pub fn compile(&mut self) -> Result<(), FleetError> {
        self.compiled = Some(build_globset(&self.patterns, "protected_paths.patterns")?);
        Ok(())
    }

    pub fn is_protected(&self, path: &Path) -> bool {
        self.compiled.as_ref().is_some_and(|set| {
            set.is_match(normalize_path(path))
                || set.is_match(
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
        })
    }
}

pub fn is_generated(path: &Path, content: &str) -> bool {
    let normalized = normalize_path(path).to_ascii_lowercase();
    if [
        "/generated/",
        "/gen/",
        "/vendor/",
        "/dist/",
        "/build/",
        "/target/",
        "/node_modules/",
    ]
    .iter()
    .any(|segment| normalized.contains(segment))
        || normalized.ends_with(".g.dart")
        || normalized.ends_with(".freezed.dart")
        || normalized.ends_with(".designer.cs")
    {
        return true;
    }
    let header = content.lines().take(12).collect::<Vec<_>>().join("\n");
    header.contains("@generated")
        || header.contains("Code generated")
        || header.contains("DO NOT EDIT")
        || header.contains("auto-generated")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRepository {
    pub name: String,
    pub path: PathBuf,
}

fn build_globset(patterns: &[String], context: &str) -> Result<GlobSet, FleetError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|error| {
            FleetError::InvalidConfig(format!("invalid glob in {context}: {pattern}: {error}"))
        })?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|error| FleetError::InvalidConfig(format!("cannot compile {context}: {error}")))
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn default_fleet_name() -> String {
    "fleet".to_owned()
}

fn default_workspace_root() -> PathBuf {
    PathBuf::from(".")
}

fn default_branch_prefix() -> String {
    "codeforge/".to_owned()
}

const fn default_true() -> bool {
    true
}

const fn default_max_changed_files() -> usize {
    40
}

const fn default_max_changed_lines() -> usize {
    2500
}

fn default_protected_patterns() -> Vec<String> {
    [
        "LICENSE*",
        "THIRD_PARTY*",
        "NOTICE*",
        "SECURITY*",
        "Cargo.lock",
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "vendor/**",
        "generated/**",
        "dist/**",
        "build/**",
        "target/**",
        "node_modules/**",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_resolves_relative_workspace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo");
        let config_path = temp.path().join("fleet.toml");
        std::fs::write(
            &config_path,
            r#"
[fleet]
name = "test"
workspace_root = "."
repositories = ["repo"]
"#,
        )
        .expect("config");
        let config = FleetConfig::load(&config_path).expect("load");
        assert_eq!(config.repositories().expect("repositories").len(), 1);
    }

    #[test]
    fn protected_paths_match_lockfiles_and_directories() {
        let mut paths = ProtectedPaths::default();
        paths.compile().expect("compile");
        assert!(paths.is_protected(Path::new("Cargo.lock")));
        assert!(paths.is_protected(Path::new("generated/models.rs")));
        assert!(!paths.is_protected(Path::new("src/lib.rs")));
    }
}
