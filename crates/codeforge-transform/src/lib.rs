//! Preview-first transactional editing, unified patch generation, and persistent undo.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use codeforge_protocol::{
    BenchmarkResult, FilePatch, Patch, TextEdit, TransactionFile, TransactionRecord,
    Transformation, VerificationResult,
};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("workspace path does not exist: {0}")]
    MissingWorkspace(PathBuf),
    #[error("edit target is outside the workspace: {0}")]
    OutsideWorkspace(PathBuf),
    #[error("edit range {start}..{end} is outside file {file}")]
    RangeOutsideFile {
        file: PathBuf,
        start: usize,
        end: usize,
    },
    #[error("edit ranges overlap in {0}")]
    OverlappingEdits(PathBuf),
    #[error("edit offsets do not fall on UTF-8 character boundaries in {0}")]
    InvalidUtf8Boundary(PathBuf),
    #[error("file changed after preview: {0}")]
    StalePreview(PathBuf),
    #[error("verification failed; apply was rejected unless force is enabled")]
    VerificationRejected,
    #[error("transaction not found: {0}")]
    TransactionNotFound(String),
    #[error("transaction cannot be undone because workspace files changed: {0}")]
    UndoConflict(PathBuf),
    #[error("transaction data is corrupt: {0}")]
    CorruptTransaction(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct PreparedFile {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub before: String,
    pub after: String,
    pub before_hash: String,
    pub after_hash: String,
}

#[derive(Debug, Clone)]
pub struct PreparedChange {
    pub patch: Patch,
    pub files: Vec<PreparedFile>,
}

#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct TransactionManager {
    workspace_root: PathBuf,
}

impl TransactionManager {
    pub fn new(workspace_root: impl AsRef<Path>) -> Result<Self, TransformError> {
        let root = workspace_root.as_ref();
        if !root.is_dir() {
            return Err(TransformError::MissingWorkspace(root.to_path_buf()));
        }
        Ok(Self {
            workspace_root: root.canonicalize()?,
        })
    }

    pub fn preview(
        &self,
        transformations: &[Transformation],
    ) -> Result<PreparedChange, TransformError> {
        let mut grouped = BTreeMap::<PathBuf, Vec<TextEdit>>::new();
        for transformation in transformations {
            for file_edit in &transformation.edits {
                let path = self.resolve_and_validate(&file_edit.file)?;
                grouped
                    .entry(path)
                    .or_default()
                    .extend(file_edit.edits.clone());
            }
        }

        let mut prepared_files = Vec::with_capacity(grouped.len());
        let mut file_patches = Vec::with_capacity(grouped.len());
        for (path, mut edits) in grouped {
            edits.sort_by_key(|edit| edit.range.start_byte);
            validate_edits(&path, &edits)?;
            let before = fs::read_to_string(&path)?;
            let after = apply_edits(&path, &before, &edits)?;
            let before_hash = hash_text(&before);
            let after_hash = hash_text(&after);
            let diff = TextDiff::from_lines(&before, &after);
            let mut additions = 0usize;
            let mut deletions = 0usize;
            for change in diff.iter_all_changes() {
                match change.tag() {
                    ChangeTag::Insert => additions += 1,
                    ChangeTag::Delete => deletions += 1,
                    ChangeTag::Equal => {}
                }
            }
            let relative_path = path
                .strip_prefix(&self.workspace_root)
                .unwrap_or(&path)
                .to_path_buf();
            let unified_diff = diff
                .unified_diff()
                .header(
                    &format!("a/{}", relative_path.to_string_lossy().replace('\\', "/")),
                    &format!("b/{}", relative_path.to_string_lossy().replace('\\', "/")),
                )
                .to_string();
            file_patches.push(FilePatch {
                file: relative_path.clone(),
                before_hash: before_hash.clone(),
                after_hash: after_hash.clone(),
                additions,
                deletions,
                unified_diff,
            });
            prepared_files.push(PreparedFile {
                path,
                relative_path,
                before,
                after,
                before_hash,
                after_hash,
            });
        }

        let additions = file_patches.iter().map(|patch| patch.additions).sum();
        let deletions = file_patches.iter().map(|patch| patch.deletions).sum();
        let patch = Patch {
            id: Uuid::new_v4().to_string(),
            transformation_ids: transformations
                .iter()
                .map(|transformation| transformation.id.clone())
                .collect(),
            files: file_patches,
            additions,
            deletions,
        };
        Ok(PreparedChange {
            patch,
            files: prepared_files,
        })
    }

    pub fn apply(
        &self,
        prepared: &PreparedChange,
        title: impl Into<String>,
        verification: VerificationResult,
        benchmarks: Vec<BenchmarkResult>,
        options: ApplyOptions,
    ) -> Result<TransactionRecord, TransformError> {
        if !options.force && !verification.accepted() {
            return Err(TransformError::VerificationRejected);
        }

        for file in &prepared.files {
            let current = fs::read_to_string(&file.path)?;
            if hash_text(&current) != file.before_hash {
                return Err(TransformError::StalePreview(file.relative_path.clone()));
            }
        }

        let transaction = TransactionRecord {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            title: title.into(),
            patch: prepared.patch.clone(),
            verification,
            benchmarks,
            files: prepared
                .files
                .iter()
                .map(|file| TransactionFile {
                    path: file.relative_path.clone(),
                    before_hash: file.before_hash.clone(),
                    after_hash: file.after_hash.clone(),
                    before: file.before.clone(),
                    after: file.after.clone(),
                })
                .collect(),
        };

        self.write_history(&transaction)?;
        let mut written = Vec::<PathBuf>::new();
        for file in &prepared.files {
            if let Err(error) = fs::write(&file.path, &file.after) {
                for written_path in written.into_iter().rev() {
                    if let Some(original) = prepared
                        .files
                        .iter()
                        .find(|candidate| candidate.path == written_path)
                    {
                        let _ = fs::write(&written_path, &original.before);
                    }
                }
                let _ = fs::remove_file(self.history_path(&transaction.id));
                return Err(error.into());
            }
            written.push(file.path.clone());
        }
        Ok(transaction)
    }

    pub fn undo(&self, transaction_id: &str) -> Result<TransactionRecord, TransformError> {
        let transaction = self.load_history(transaction_id)?;
        for file in &transaction.files {
            let path = self.workspace_root.join(&file.path);
            let current = fs::read_to_string(&path)?;
            if hash_text(&current) != file.after_hash {
                return Err(TransformError::UndoConflict(file.path.clone()));
            }
        }
        let mut restored = Vec::new();
        for file in &transaction.files {
            let path = self.workspace_root.join(&file.path);
            if let Err(error) = fs::write(&path, &file.before) {
                for restored_path in restored.into_iter().rev() {
                    if let Some(original) = transaction.files.iter().find(|candidate| {
                        self.workspace_root.join(&candidate.path) == restored_path
                    }) {
                        let _ = fs::write(&restored_path, &original.after);
                    }
                }
                return Err(error.into());
            }
            restored.push(path);
        }
        fs::remove_file(self.history_path(transaction_id))?;
        Ok(transaction)
    }

    pub fn history(&self) -> Result<Vec<TransactionRecord>, TransformError> {
        let directory = self.history_dir();
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            match serde_json::from_str::<TransactionRecord>(&fs::read_to_string(entry.path())?) {
                Ok(record) => records.push(record),
                Err(error) => {
                    tracing::warn!(%error, path = %entry.path().display(), "skipping corrupt transaction history")
                }
            }
        }
        records.sort_by_key(|left| std::cmp::Reverse(left.created_at));
        Ok(records)
    }

    pub fn history_dir(&self) -> PathBuf {
        self.workspace_root.join(".codeforge").join("history")
    }

    pub fn history_path(&self, transaction_id: &str) -> PathBuf {
        self.history_dir().join(format!("{transaction_id}.json"))
    }

    fn resolve_and_validate(&self, path: &Path) -> Result<PathBuf, TransformError> {
        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_root.join(path)
        };
        let parent = candidate
            .parent()
            .ok_or_else(|| TransformError::OutsideWorkspace(candidate.clone()))?;
        let canonical_parent = parent.canonicalize()?;
        if !canonical_parent.starts_with(&self.workspace_root) {
            return Err(TransformError::OutsideWorkspace(candidate));
        }
        let name = candidate
            .file_name()
            .ok_or_else(|| TransformError::OutsideWorkspace(candidate.clone()))?;
        Ok(canonical_parent.join(name))
    }

    fn write_history(&self, transaction: &TransactionRecord) -> Result<(), TransformError> {
        fs::create_dir_all(self.history_dir())?;
        let path = self.history_path(&transaction.id);
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(transaction)?)?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    fn load_history(&self, transaction_id: &str) -> Result<TransactionRecord, TransformError> {
        let path = self.history_path(transaction_id);
        if !path.exists() {
            return Err(TransformError::TransactionNotFound(
                transaction_id.to_owned(),
            ));
        }
        let record: TransactionRecord = serde_json::from_slice(&fs::read(path)?)?;
        if record.id != transaction_id {
            return Err(TransformError::CorruptTransaction(
                transaction_id.to_owned(),
            ));
        }
        Ok(record)
    }
}

fn validate_edits(path: &Path, edits: &[TextEdit]) -> Result<(), TransformError> {
    for pair in edits.windows(2) {
        if pair[0].range.end_byte > pair[1].range.start_byte {
            return Err(TransformError::OverlappingEdits(path.to_path_buf()));
        }
    }
    Ok(())
}

fn apply_edits(path: &Path, source: &str, edits: &[TextEdit]) -> Result<String, TransformError> {
    let mut output = source.to_owned();
    for edit in edits.iter().rev() {
        let start = edit.range.start_byte;
        let end = edit.range.end_byte;
        if end > source.len() {
            return Err(TransformError::RangeOutsideFile {
                file: path.to_path_buf(),
                start,
                end,
            });
        }
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err(TransformError::InvalidUtf8Boundary(path.to_path_buf()));
        }
        output.replace_range(start..end, &edit.replacement);
    }
    Ok(output)
}

fn hash_text(source: &str) -> String {
    let digest = Sha256::digest(source.as_bytes());
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use codeforge_protocol::{
        Confidence, DiagnosticCategory, FileEdit, Language, Severity, SourceRange,
    };

    #[test]
    fn apply_and_undo_restore_original() {
        let temp = tempfile::tempdir().expect("tempdir");
        let file = temp.path().join("example.py");
        fs::write(&file, "if value == None:\n    pass\n").expect("write");
        let range =
            SourceRange::from_offsets("if value == None:\n    pass\n", 3, 16).expect("range");
        let transformation = Transformation::new(
            "test",
            Language::Python,
            "Use is None",
            "Python identity comparison",
            vec![FileEdit {
                file: PathBuf::from("example.py"),
                edits: vec![TextEdit {
                    range,
                    replacement: "value is None".to_owned(),
                    description: None,
                }],
            }],
        );
        let manager = TransactionManager::new(temp.path()).expect("manager");
        let prepared = manager.preview(&[transformation]).expect("preview");
        let verification = VerificationResult {
            syntax: codeforge_protocol::VerificationCheck::passed("ok", 1),
            ..VerificationResult::default()
        };
        let transaction = manager
            .apply(
                &prepared,
                "test",
                verification,
                Vec::new(),
                ApplyOptions::default(),
            )
            .expect("apply");
        assert_eq!(
            fs::read_to_string(&file).expect("read"),
            "if value is None:\n    pass\n"
        );
        manager.undo(&transaction.id).expect("undo");
        assert_eq!(
            fs::read_to_string(&file).expect("read"),
            "if value == None:\n    pass\n"
        );
        let _ = (
            Severity::Warning,
            DiagnosticCategory::Correctness,
            Confidence::High,
        );
    }
}
