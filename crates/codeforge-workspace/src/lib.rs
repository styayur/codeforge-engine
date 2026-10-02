//! Workspace discovery, language detection, file hashing, and lightweight indexing.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::SystemTime;

use codeforge_protocol::{Language, LanguageStats, Result, WorkspaceSummary};
use ignore::WalkBuilder;
use sha2::{Digest, Sha256};

const DEFAULT_MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const PROJECT_MARKERS: &[&str] = &[
    "pyproject.toml",
    "requirements.txt",
    "Cargo.toml",
    "CMakeLists.txt",
    "Makefile",
    "package.json",
    "tsconfig.json",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "go.mod",
    "pubspec.yaml",
    "fleet.toml",
    ".codeforge.toml",
];

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace path does not exist: {0}")]
    Missing(PathBuf),
    #[error("workspace path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("cannot read workspace: {0}")]
    Io(#[from] std::io::Error),
    #[error("workspace file exceeds configured limit of {limit} bytes: {path}")]
    FileTooLarge { path: PathBuf, limit: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub language: Option<Language>,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub hash: String,
}

#[derive(Debug)]
pub struct WorkspaceManager {
    root: PathBuf,
    project_markers: Vec<String>,
    records: RwLock<HashMap<PathBuf, FileRecord>>,
    max_file_bytes: u64,
}

impl WorkspaceManager {
    pub fn open(root: impl AsRef<Path>) -> std::result::Result<Self, WorkspaceError> {
        let root = root.as_ref();
        if !root.exists() {
            return Err(WorkspaceError::Missing(root.to_path_buf()));
        }
        if !root.is_dir() {
            return Err(WorkspaceError::NotDirectory(root.to_path_buf()));
        }
        let root = root.canonicalize()?;
        let project_markers = discover_project_markers(&root);
        let manager = Self {
            root,
            project_markers,
            records: RwLock::new(HashMap::new()),
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        };
        manager.scan()?;
        Ok(manager)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn records(&self) -> Vec<FileRecord> {
        self.records
            .read()
            .map(|records| records.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn files_for_language(&self, language: Language) -> Vec<PathBuf> {
        let mut files = self
            .records()
            .into_iter()
            .filter(|record| record.language == Some(language))
            .map(|record| record.path)
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    pub fn language_stats(&self) -> BTreeMap<Language, LanguageStats> {
        let mut stats = BTreeMap::<Language, LanguageStats>::new();
        for record in self.records() {
            if let Some(language) = record.language {
                let entry = stats.entry(language).or_default();
                entry.files += 1;
                entry.bytes += record.size;
            }
        }
        stats
    }

    pub fn summary(&self) -> WorkspaceSummary {
        let root = self.root.clone();
        WorkspaceSummary {
            name: root
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("workspace")
                .to_owned(),
            languages: self.language_stats(),
            project_markers: self.project_markers.clone(),
            git_repository: self.root.join(".git").exists(),
            git_branch: None,
            root,
        }
    }

    pub fn scan(&self) -> std::result::Result<Vec<FileRecord>, WorkspaceError> {
        let mut builder = WalkBuilder::new(&self.root);
        builder
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .ignore(true)
            .parents(true)
            .follow_links(false)
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                !matches!(
                    name.as_ref(),
                    ".git" | ".codeforge" | "target" | "node_modules" | "dist" | "build"
                )
            });

        let mut records = HashMap::new();
        for entry in builder.build() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    tracing::warn!(%error, root = %self.root.display(), "skipping unreadable workspace entry");
                    continue;
                }
            };
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                continue;
            }
            let path = entry.path();
            let relative_path = path.strip_prefix(&self.root).unwrap_or(path).to_path_buf();
            let language = path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(Language::from_extension);
            if language.is_none() && !is_known_project_marker(path) {
                continue;
            }
            match self.file_record(path, relative_path, language) {
                Ok(record) => {
                    records.insert(record.path.clone(), record);
                }
                Err(WorkspaceError::FileTooLarge { .. }) => {
                    tracing::debug!(path = %path.display(), "skipping large workspace file");
                }
                Err(error) => return Err(error),
            }
        }

        if let Ok(mut cache) = self.records.write() {
            *cache = records.clone();
        }
        let mut values = records.drain().map(|(_, value)| value).collect::<Vec<_>>();
        values.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(values)
    }

    pub fn refresh_file(&self, path: impl AsRef<Path>) -> Result<FileRecord, WorkspaceError> {
        let path = path.as_ref();
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let relative = absolute
            .strip_prefix(&self.root)
            .unwrap_or(&absolute)
            .to_path_buf();
        let language = absolute
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(Language::from_extension);
        let record = self.file_record(&absolute, relative, language)?;
        if let Ok(mut cache) = self.records.write() {
            cache.insert(absolute, record.clone());
        }
        Ok(record)
    }

    pub fn read_text(&self, path: impl AsRef<Path>) -> std::result::Result<String, WorkspaceError> {
        let path = self.resolve(path);
        let metadata = fs::metadata(&path)?;
        if metadata.len() > self.max_file_bytes {
            return Err(WorkspaceError::FileTooLarge {
                path,
                limit: self.max_file_bytes,
            });
        }
        Ok(fs::read_to_string(path)?)
    }

    pub fn resolve(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }

    pub fn relative(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        path.strip_prefix(&self.root).unwrap_or(path).to_path_buf()
    }

    pub fn set_max_file_bytes(&mut self, limit: u64) {
        self.max_file_bytes = limit.max(1);
    }

    fn file_record(
        &self,
        path: &Path,
        relative_path: PathBuf,
        language: Option<Language>,
    ) -> std::result::Result<FileRecord, WorkspaceError> {
        let metadata = fs::metadata(path)?;
        if metadata.len() > self.max_file_bytes {
            return Err(WorkspaceError::FileTooLarge {
                path: path.to_path_buf(),
                limit: self.max_file_bytes,
            });
        }
        let bytes = fs::read(path)?;
        Ok(FileRecord {
            path: path.to_path_buf(),
            relative_path,
            language,
            size: metadata.len(),
            modified: metadata.modified().ok(),
            hash: hash_bytes(&bytes),
        })
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

pub fn hash_file(path: impl AsRef<Path>) -> std::io::Result<String> {
    Ok(hash_bytes(&fs::read(path)?))
}

pub fn detect_languages(root: impl AsRef<Path>) -> std::io::Result<Vec<Language>> {
    let root = root.as_ref();
    let mut languages = HashSet::new();
    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        languages.insert(Language::Python);
    }
    if root.join("Cargo.toml").exists() {
        languages.insert(Language::Rust);
    }
    if root.join("CMakeLists.txt").exists() || root.join("Makefile").exists() {
        languages.insert(Language::C);
    }
    if root.join("package.json").exists() || root.join("tsconfig.json").exists() {
        languages.insert(Language::JavaScript);
        if root.join("tsconfig.json").exists() {
            languages.insert(Language::TypeScript);
        }
    }
    if root.join("pom.xml").exists()
        || root.join("build.gradle").exists()
        || root.join("build.gradle.kts").exists()
    {
        languages.insert(Language::Java);
    }
    if root.join("go.mod").exists() {
        languages.insert(Language::Go);
    }
    if root.join("pubspec.yaml").exists() {
        languages.insert(Language::Dart);
    }
    let manager =
        WorkspaceManager::open(root).map_err(|error| std::io::Error::other(error.to_string()))?;
    for record in manager.records() {
        if let Some(language) = record.language {
            languages.insert(language);
        }
    }
    let mut ordered = languages.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|language| language.as_str());
    Ok(ordered)
}

fn discover_project_markers(root: &Path) -> Vec<String> {
    PROJECT_MARKERS
        .iter()
        .filter(|marker| root.join(marker).exists())
        .map(|marker| (*marker).to_owned())
        .collect()
}

fn is_known_project_marker(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| PROJECT_MARKERS.contains(&name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn indexes_supported_source_files_and_detects_project() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname='x'\n").expect("manifest");
        fs::write(temp.path().join("src.rs"), "fn main() {}\n").expect("source");
        let manager = WorkspaceManager::open(temp.path()).expect("workspace");
        assert_eq!(manager.files_for_language(Language::Rust).len(), 1);
        assert!(
            manager
                .summary()
                .project_markers
                .contains(&"Cargo.toml".to_owned())
        );
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(hash_bytes(b"forge"), hash_bytes(b"forge"));
        assert_ne!(hash_bytes(b"forge"), hash_bytes(b"forge2"));
    }
}
