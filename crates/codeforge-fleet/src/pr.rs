use std::path::{Path, PathBuf};
use std::process::Stdio;

use codeforge_engines::find_executable;
use codeforge_git::GitRepository;
use serde::{Deserialize, Serialize};

use crate::FleetError;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrMode {
    #[default]
    Disabled,
    BranchOnly,
    Open {
        title: String,
        body: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrOutcome {
    Disabled,
    BranchCreated { branch: String },
    PullRequestCreated { branch: String, url: String },
    Unavailable { branch: String, reason: String },
}

#[derive(Debug, Clone, Default)]
pub struct PrManager;

impl PrManager {
    pub fn branch_name(prefix: &str, task: &str) -> String {
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        format!(
            "{}{}-{}-{}",
            prefix,
            sanitize(task),
            timestamp,
            &suffix[..8]
        )
    }

    pub async fn create_branch(
        &self,
        repository_root: &Path,
        branch: &str,
        require_clean_tree: bool,
    ) -> Result<(), FleetError> {
        let repository = GitRepository::discover(repository_root)?;
        if require_clean_tree && !repository.is_clean().await? {
            return Err(FleetError::SafetyRefusal {
                repository: repository_root.display().to_string(),
                reason: "working tree is dirty".to_owned(),
            });
        }
        repository.create_branch(branch).await?;
        Ok(())
    }

    pub async fn publish(
        &self,
        repository_root: &Path,
        branch: &str,
        paths: &[PathBuf],
        mode: &PrMode,
    ) -> Result<PrOutcome, FleetError> {
        if matches!(mode, PrMode::Disabled) {
            return Ok(PrOutcome::Disabled);
        }
        let repository = GitRepository::discover(repository_root)?;
        repository
            .stage_paths(paths.iter().map(PathBuf::as_path))
            .await?;
        repository
            .commit("chore(codeforge): safe refactoring pass")
            .await?;
        repository.push_branch(branch).await?;

        let PrMode::Open { title, body } = mode else {
            return Ok(PrOutcome::BranchCreated {
                branch: branch.to_owned(),
            });
        };
        let Some(gh) = find_executable("gh") else {
            return Ok(PrOutcome::Unavailable {
                branch: branch.to_owned(),
                reason: "PR creation unavailable: gh CLI was not found".to_owned(),
            });
        };
        let output = tokio::process::Command::new(gh)
            .args([
                "pr", "create", "--title", title, "--body", body, "--head", branch,
            ])
            .current_dir(repository_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output()
            .await?;
        if !output.status.success() {
            return Ok(PrOutcome::Unavailable {
                branch: branch.to_owned(),
                reason: format!(
                    "PR creation unavailable: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            });
        }
        Ok(PrOutcome::PullRequestCreated {
            branch: branch.to_owned(),
            url: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        })
    }
}

fn sanitize(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while output.contains("--") {
        output = output.replace("--", "-");
    }
    output.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_names_are_stable_safe_and_unique() {
        let left = PrManager::branch_name("codeforge/", "Fleet Mode v0.2");
        let right = PrManager::branch_name("codeforge/", "Fleet Mode v0.2");
        assert!(left.starts_with("codeforge/fleet-mode-v0-2-"));
        assert_ne!(left, right);
    }
}
