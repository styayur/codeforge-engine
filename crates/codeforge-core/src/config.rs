use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use codeforge_verification::CommandSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeForgeConfig {
    #[serde(default)]
    pub commands: BTreeMap<String, CommandConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_cwd")]
    pub cwd: PathBuf,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_cwd() -> PathBuf {
    PathBuf::from(".")
}

const fn default_timeout() -> u64 {
    300
}

impl CodeForgeConfig {
    pub fn load(root: &Path) -> Result<Self, ConfigError> {
        let path = root.join(".codeforge.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(|source| ConfigError::Parse { path, source })
    }

    pub fn command(&self, root: &Path, id: &str) -> Option<CommandSpec> {
        self.commands.get(id).map(|command| {
            let cwd = if command.cwd.is_absolute() {
                command.cwd.clone()
            } else {
                root.join(&command.cwd)
            };
            CommandSpec::new(&command.program, command.args.iter().cloned(), cwd)
                .label(id)
                .timeout(Duration::from_secs(command.timeout_secs.max(1)))
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read project configuration {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse project configuration {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

impl From<std::io::Error> for ConfigError {
    fn from(source: std::io::Error) -> Self {
        Self::Read {
            path: PathBuf::from(".codeforge.toml"),
            source,
        }
    }
}
