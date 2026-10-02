use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use codeforge_protocol::RepoRunSummary;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::FleetError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheKey {
    pub repository: String,
    pub commit: Option<String>,
    pub config_hash: String,
    pub tool_version: String,
    pub rule_version: String,
}

impl CacheKey {
    pub fn new(
        repository: impl Into<String>,
        commit: Option<String>,
        config_hash: impl Into<String>,
        tool_version: impl Into<String>,
        rule_version: impl Into<String>,
    ) -> Self {
        Self {
            repository: repository.into(),
            commit,
            config_hash: config_hash.into(),
            tool_version: tool_version.into(),
            rule_version: rule_version.into(),
        }
    }

    pub fn stable_id(&self) -> String {
        let serialized = serde_json::to_vec(self).unwrap_or_default();
        format!("{:x}", sha2::Sha256::digest(serialized))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetCache {
    #[serde(skip)]
    path: PathBuf,
    #[serde(default)]
    entries: BTreeMap<String, RepoRunSummary>,
}

impl FleetCache {
    pub fn load(root: &Path) -> Result<Self, FleetError> {
        let path = root.join("cache.json");
        if !path.exists() {
            return Ok(Self {
                path,
                entries: BTreeMap::new(),
            });
        }
        let mut cache: Self = serde_json::from_slice(&std::fs::read(&path)?)?;
        cache.path = path;
        Ok(cache)
    }

    pub fn get(&self, key: &CacheKey) -> Option<RepoRunSummary> {
        self.entries.get(&key.stable_id()).cloned()
    }

    pub fn insert(&mut self, key: CacheKey, summary: RepoRunSummary) {
        self.entries.insert(key.stable_id(), summary);
    }

    pub fn save(&self) -> Result<(), FleetError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temporary, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable_and_sensitive_to_inputs() {
        let left = CacheKey::new("repo", Some("abc".to_owned()), "cfg", "tool", "rule");
        let right = left.clone();
        let changed = CacheKey::new("repo", Some("def".to_owned()), "cfg", "tool", "rule");
        assert_eq!(left.stable_id(), right.stable_id());
        assert_ne!(left.stable_id(), changed.stable_id());
    }
}
