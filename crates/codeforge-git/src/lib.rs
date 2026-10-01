//! Structured Git integration without shell interpolation.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("not a Git repository: {0}")]
    NotRepository(PathBuf),
    #[error("git command failed: {0}")]
    CommandFailed(String),
    #[error("git output was not valid UTF-8")]
    OutputEncoding,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub index_status: char,
    pub worktree_status: char,
    pub untracked: bool,
}

impl ChangedFile {
    pub fn is_staged(&self) -> bool {
        self.index_status != ' ' && self.index_status != '?'
    }

    pub fn is_unstaged(&self) -> bool {
        self.worktree_status != ' ' || self.untracked
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffScope<'a> {
    Worktree,
    Staged,
    Range { from: &'a str, to: &'a str },
}

#[derive(Debug, Clone)]
pub struct GitRepository {
    root: PathBuf,
    timeout: Duration,
}

impl GitRepository {
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, GitError> {
        let path = path.as_ref();
        let output = std::process::Command::new("git")
            .args(["-C"])
            .arg(path)
            .args(["rev-parse", "--show-toplevel"])
            .output()?;
        if !output.status.success() {
            return Err(GitError::NotRepository(path.to_path_buf()));
        }
        let root = String::from_utf8(output.stdout)
            .map_err(|_| GitError::OutputEncoding)?
            .trim()
            .to_owned();
        Ok(Self {
            root: PathBuf::from(root),
            timeout: Duration::from_secs(30),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub async fn branch(&self) -> Result<String, GitError> {
        let output = self.run(["rev-parse", "--abbrev-ref", "HEAD"]).await?;
        Ok(output.trim().to_owned())
    }

    pub async fn changed_files(&self) -> Result<Vec<ChangedFile>, GitError> {
        let output = self
            .run(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
            .await?;
        let mut files = Vec::new();
        let mut parts = output.split('\0');
        while let Some(entry) = parts.next() {
            if entry.is_empty() {
                continue;
            }
            let bytes = entry.as_bytes();
            if bytes.len() < 4 {
                continue;
            }
            let index_status = bytes[0] as char;
            let worktree_status = bytes[1] as char;
            let path = PathBuf::from(&entry[3..]);
            if index_status == 'R' && worktree_status == 'R' {
                // Porcelain v1 -z emits original path as the next NUL field.
                let _ = parts.next();
            }
            files.push(ChangedFile {
                path,
                index_status,
                worktree_status,
                untracked: index_status == '?' && worktree_status == '?',
            });
        }
        Ok(files)
    }

    pub async fn diff(&self, scope: DiffScope<'_>) -> Result<String, GitError> {
        let mut args = vec!["diff", "--no-ext-diff", "--no-color", "--unified=3"];
        match scope {
            DiffScope::Worktree => {}
            DiffScope::Staged => args.push("--cached"),
            DiffScope::Range { from, to } => {
                args.push(from);
                args.push(to);
            }
        }
        self.run(args).await
    }

    async fn run<I, S>(&self, args: I) -> Result<String, GitError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let future = command.output();
        let output = tokio::time::timeout(self.timeout, future)
            .await
            .map_err(|_| GitError::CommandFailed("git command timed out".to_owned()))??;
        if !output.status.success() {
            return Err(GitError::CommandFailed(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        String::from_utf8(output.stdout).map_err(|_| GitError::OutputEncoding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn discovers_repository_and_lists_changes() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(temp.path())
            .status()
            .expect("git init");
        std::fs::write(temp.path().join("changed.txt"), "content").expect("write");
        let repository = GitRepository::discover(temp.path()).expect("repository");
        let changed = repository.changed_files().await.expect("status");
        assert!(
            changed
                .iter()
                .any(|file| file.path == Path::new("changed.txt"))
        );
    }
}
