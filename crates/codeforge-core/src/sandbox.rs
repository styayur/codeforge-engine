use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

const MAX_SANDBOX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("failed to create sandbox: {0}")]
    Temp(#[from] std::io::Error),
    #[error("sandbox copy would exceed {limit} bytes after adding {path}")]
    TooLarge { path: PathBuf, limit: u64 },
}

pub struct Sandbox {
    _temp: tempfile::TempDir,
    root: PathBuf,
}

impl Sandbox {
    pub fn create(source: &Path) -> Result<Self, SandboxError> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("workspace");
        std::fs::create_dir_all(&root)?;
        copy_workspace(source, &root)?;
        Ok(Self { _temp: temp, root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn copy_workspace(source: &Path, destination: &Path) -> Result<(), SandboxError> {
    let mut walked = WalkBuilder::new(source);
    walked
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .ignore(true)
        .follow_links(false)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(name.as_ref(), ".git" | ".codeforge" | "target")
        });

    let mut total = 0u64;
    for entry in walked.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!(%error, source = %source.display(), "skipping unreadable sandbox entry");
                continue;
            }
        };
        let relative = match entry.path().strip_prefix(source) {
            Ok(relative) => relative,
            Err(_) => continue,
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = destination.join(relative);
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        total = total.saturating_add(metadata.len());
        if total > MAX_SANDBOX_BYTES {
            return Err(SandboxError::TooLarge {
                path: entry.path().to_path_buf(),
                limit: MAX_SANDBOX_BYTES,
            });
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(entry.path(), target)?;
    }
    Ok(())
}
