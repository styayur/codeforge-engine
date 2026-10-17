use codeforge_verification::{CommandSpec, VerificationPipeline};
use sha2::Digest;
use std::path::{Path, PathBuf};

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
    PullRequestReused { branch: String, url: String },
    Unavailable { branch: String, reason: String },
}

#[derive(Debug, Clone, Default)]
pub struct PrManager;

impl PrManager {
    pub fn branch_name(prefix: &str, task: &str) -> String {
        let digest = Self::task_id(&serde_json::json!(task));
        let label: String = sanitize(task).chars().take(48).collect();
        format!("{prefix}{label}-{}", &digest[..24])
    }

    /// Hash structured inputs, not concatenated strings or wall-clock time.
    pub fn task_id(inputs: &serde_json::Value) -> String {
        sha2::Sha256::digest(inputs.to_string().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
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
        if let Some(gh) = find_executable("gh")
            && let Some(url) = lookup_pr(&gh, repository_root, branch).await?
        {
            return Err(FleetError::SafetyRefusal {
                repository: repository_root.display().to_string(),
                reason: format!("task already has a PR: {url}"),
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
        let gh = if matches!(mode, PrMode::Open { .. }) {
            let Some(gh) = find_executable("gh") else {
                return Ok(PrOutcome::Unavailable {
                    branch: branch.into(),
                    reason: "gh CLI was not found".into(),
                });
            };
            if let Some(url) = lookup_pr(&gh, repository_root, branch).await? {
                return Ok(PrOutcome::PullRequestReused {
                    branch: branch.into(),
                    url,
                });
            }
            Some(gh)
        } else {
            None
        };
        let repository = GitRepository::discover(repository_root)?;
        if repository.branch().await? != branch {
            return Err(FleetError::SafetyRefusal {
                repository: repository_root.display().to_string(),
                reason: "publication branch differs from checked-out branch".into(),
            });
        }
        repository
            .stage_paths(paths.iter().map(PathBuf::as_path))
            .await?;
        // Retry after a successful commit/push must not create an empty commit.
        if !repository
            .diff(codeforge_git::DiffScope::Staged)
            .await?
            .is_empty()
        {
            repository
                .commit("chore(codeforge): safe refactoring pass")
                .await?;
        }
        repository.push_branch(branch).await?;
        let PrMode::Open { title, body } = mode else {
            return Ok(PrOutcome::BranchCreated {
                branch: branch.to_owned(),
            });
        };
        let gh = gh.expect("open mode resolves gh before any mutation");
        let output = VerificationPipeline
            .run_command(
                CommandSpec::new(
                    &gh,
                    [
                        "pr", "create", "--title", title, "--body", body, "--head", branch,
                    ],
                    repository_root,
                )
                .timeout(std::time::Duration::from_secs(60)),
            )
            .await?;
        if !output.success() || output.output_truncated {
            return Ok(PrOutcome::Unavailable {
                branch: branch.to_owned(),
                reason: format!("PR creation unavailable: {}", output.stderr.trim()),
            });
        }
        Ok(PrOutcome::PullRequestCreated {
            branch: branch.to_owned(),
            url: output.stdout.trim().to_owned(),
        })
    }
}

async fn lookup_pr(gh: &Path, root: &Path, branch: &str) -> Result<Option<String>, FleetError> {
    let lookup = VerificationPipeline
        .run_command(
            CommandSpec::new(
                gh,
                [
                    "pr", "list", "--state", "all", "--head", branch, "--json", "url", "--limit",
                    "2",
                ],
                root,
            )
            .timeout(std::time::Duration::from_secs(30)),
        )
        .await?;
    if !lookup.success() || lookup.output_truncated {
        return Err(FleetError::SafetyRefusal {
            repository: root.display().to_string(),
            reason: "PR lookup failed; refusing duplicate creation".into(),
        });
    }
    existing_pr_url(&lookup.stdout)
}

fn existing_pr_url(json: &str) -> Result<Option<String>, FleetError> {
    #[derive(Deserialize)]
    struct ExistingPr {
        url: String,
    }
    let prs: Vec<ExistingPr> = serde_json::from_str(json)?;
    if prs.len() > 1 || prs.iter().any(|pr| !pr.url.starts_with("https://")) {
        return Err(FleetError::InvalidConfig("ambiguous PR lookup".into()));
    }
    Ok(prs.into_iter().next().map(|pr| pr.url))
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
    fn task_keys_and_pr_lookup_are_deterministic() {
        let a = serde_json::json!({"repository":"a", "revision":"1", "policy":"safe"});
        assert_eq!(PrManager::task_id(&a), PrManager::task_id(&a));
        assert_ne!(
            PrManager::task_id(&a),
            PrManager::task_id(
                &serde_json::json!({"repository":"a", "revision":"2", "policy":"safe"})
            )
        );
        assert_eq!(existing_pr_url("[]").unwrap(), None);
        assert_eq!(
            existing_pr_url(r#"[{"url":"https://github.com/o/r/pull/1"}]"#)
                .unwrap()
                .as_deref(),
            Some("https://github.com/o/r/pull/1")
        );
        assert!(existing_pr_url("{}").is_err());
        assert!(existing_pr_url(r#"[{"url":"https://a"},{"url":"https://b"}]"#).is_err());
    }

    #[test]
    fn branch_names_are_stable_safe_and_unique() {
        let left = PrManager::branch_name("codeforge/", "Fleet Mode v0.2");
        let right = PrManager::branch_name("codeforge/", "Fleet Mode v0.2");
        assert!(left.starts_with("codeforge/fleet-mode-v0-2-"));
        assert_eq!(left, right);
        assert_ne!(
            left,
            PrManager::branch_name("codeforge/", "fleet-mode-v0-2")
        );
    }
}
