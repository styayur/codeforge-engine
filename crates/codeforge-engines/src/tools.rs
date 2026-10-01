use std::path::{Path, PathBuf};

use codeforge_protocol::{Capability, EngineMetadata, EngineStatus, Language, PluginPermission};

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub languages: &'static [Language],
    pub capabilities: &'static [Capability],
    pub executables: &'static [&'static str],
    pub dependencies: &'static [&'static str],
}

pub fn tool_definitions() -> &'static [ToolDefinition] {
    &[
        ToolDefinition {
            id: "python-ruff",
            name: "Ruff",
            languages: &[Language::Python],
            capabilities: &[Capability::Lint, Capability::Fix, Capability::Format],
            executables: &["ruff"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "python-pyright",
            name: "Pyright",
            languages: &[Language::Python],
            capabilities: &[Capability::Verify],
            executables: &["pyright"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "python-mypy",
            name: "mypy",
            languages: &[Language::Python],
            capabilities: &[Capability::Verify],
            executables: &["mypy"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "rust-cargo",
            name: "Cargo",
            languages: &[Language::Rust],
            capabilities: &[Capability::Verify, Capability::Benchmark],
            executables: &["cargo"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "rust-clippy",
            name: "Clippy",
            languages: &[Language::Rust],
            capabilities: &[Capability::Lint, Capability::Fix],
            executables: &["cargo-clippy"],
            dependencies: &["cargo"],
        },
        ToolDefinition {
            id: "rust-rustfmt",
            name: "rustfmt",
            languages: &[Language::Rust],
            capabilities: &[Capability::Format, Capability::Fix],
            executables: &["rustfmt"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "rust-analyzer",
            name: "rust-analyzer",
            languages: &[Language::Rust],
            capabilities: &[Capability::Parse, Capability::Refactor],
            executables: &["rust-analyzer"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "c-clang",
            name: "Clang",
            languages: &[Language::C],
            capabilities: &[Capability::Parse, Capability::Verify],
            executables: &["clang"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "c-clang-tidy",
            name: "clang-tidy",
            languages: &[Language::C],
            capabilities: &[Capability::Lint, Capability::Fix, Capability::Refactor],
            executables: &["clang-tidy"],
            dependencies: &["clang"],
        },
        ToolDefinition {
            id: "c-clang-format",
            name: "clang-format",
            languages: &[Language::C],
            capabilities: &[Capability::Format],
            executables: &["clang-format"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "c-alive2",
            name: "Alive2",
            languages: &[Language::C],
            capabilities: &[Capability::Verify],
            executables: &["alive-tv", "alive2"],
            dependencies: &["clang"],
        },
        ToolDefinition {
            id: "js-oxlint",
            name: "Oxlint",
            languages: &[Language::JavaScript, Language::TypeScript],
            capabilities: &[Capability::Lint, Capability::Fix],
            executables: &["oxlint"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "js-oxfmt",
            name: "Oxfmt",
            languages: &[Language::JavaScript, Language::TypeScript],
            capabilities: &[Capability::Format],
            executables: &["oxfmt"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "js-node",
            name: "Node.js",
            languages: &[Language::JavaScript, Language::TypeScript],
            capabilities: &[Capability::Verify, Capability::Benchmark],
            executables: &["node"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "js-pnpm",
            name: "pnpm",
            languages: &[Language::JavaScript, Language::TypeScript],
            capabilities: &[Capability::Verify, Capability::Benchmark],
            executables: &["pnpm", "pnpm.cmd"],
            dependencies: &["node"],
        },
        ToolDefinition {
            id: "java-javac",
            name: "javac",
            languages: &[Language::Java],
            capabilities: &[Capability::Verify],
            executables: &["javac"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "java-runtime",
            name: "Java Runtime",
            languages: &[Language::Java],
            capabilities: &[Capability::Verify, Capability::Benchmark],
            executables: &["java"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "java-maven",
            name: "Maven",
            languages: &[Language::Java],
            capabilities: &[
                Capability::Verify,
                Capability::Refactor,
                Capability::Benchmark,
            ],
            executables: &["mvn", "mvn.cmd"],
            dependencies: &["java"],
        },
        ToolDefinition {
            id: "java-gradle",
            name: "Gradle",
            languages: &[Language::Java],
            capabilities: &[
                Capability::Verify,
                Capability::Refactor,
                Capability::Benchmark,
            ],
            executables: &["gradle", "gradle.bat"],
            dependencies: &["java"],
        },
        ToolDefinition {
            id: "go-toolchain",
            name: "Go toolchain",
            languages: &[Language::Go],
            capabilities: &[Capability::Verify, Capability::Benchmark],
            executables: &["go"],
            dependencies: &[],
        },
        ToolDefinition {
            id: "go-gofmt",
            name: "gofmt",
            languages: &[Language::Go],
            capabilities: &[Capability::Format],
            executables: &["gofmt"],
            dependencies: &["go"],
        },
        ToolDefinition {
            id: "go-staticcheck",
            name: "Staticcheck",
            languages: &[Language::Go],
            capabilities: &[Capability::Lint],
            executables: &["staticcheck"],
            dependencies: &["go"],
        },
        ToolDefinition {
            id: "generic-ast-grep",
            name: "ast-grep",
            languages: &[
                Language::Python,
                Language::Rust,
                Language::C,
                Language::JavaScript,
                Language::TypeScript,
                Language::Java,
                Language::Go,
            ],
            capabilities: &[Capability::Refactor, Capability::Fix],
            executables: &["ast-grep", "sg"],
            dependencies: &[],
        },
    ]
}

#[derive(Debug, Clone)]
pub struct DiscoveredTool {
    pub definition: ToolDefinition,
    pub executable: Option<PathBuf>,
}

impl DiscoveredTool {
    pub fn metadata(&self) -> EngineMetadata {
        EngineMetadata {
            id: self.definition.id.to_owned(),
            name: self.definition.name.to_owned(),
            version: "local-discovery".to_owned(),
            languages: self.definition.languages.to_vec(),
            capabilities: self.definition.capabilities.to_vec(),
            executable: self.executable.clone(),
            dependencies: self
                .definition
                .dependencies
                .iter()
                .map(|dependency| (*dependency).to_owned())
                .collect(),
            timeout_ms: Some(300_000),
            permissions: vec![
                PluginPermission::ReadWorkspace,
                PluginPermission::ExecuteProcess,
            ],
        }
    }

    pub fn status(&self) -> EngineStatus {
        EngineStatus {
            metadata: self.metadata(),
            available: self.executable.is_some(),
            reason: self.executable.is_none().then(|| {
                format!(
                    "{} was not found; install it locally or configure an explicit path",
                    self.definition.executables.join(", ")
                )
            }),
        }
    }
}

pub fn discover_tools(workspace_root: &Path) -> Vec<DiscoveredTool> {
    tool_definitions()
        .iter()
        .cloned()
        .map(|definition| {
            let executable = definition
                .executables
                .iter()
                .find_map(|name| find_executable_in_workspace(workspace_root, name))
                .or_else(|| {
                    definition
                        .executables
                        .iter()
                        .find_map(|name| find_executable(name))
                });
            DiscoveredTool {
                definition,
                executable,
            }
        })
        .collect()
}

pub fn find_executable_in_workspace(workspace_root: &Path, name: &str) -> Option<PathBuf> {
    let extensions = executable_extensions();
    let directories = [
        workspace_root.join("node_modules").join(".bin"),
        workspace_root.join("venv").join("Scripts"),
        workspace_root.join(".venv").join("Scripts"),
        workspace_root.join("venv").join("bin"),
        workspace_root.join(".venv").join("bin"),
        workspace_root.join("tools"),
    ];
    for directory in directories {
        for extension in &extensions {
            let candidate = directory.join(format!("{name}{extension}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn find_executable(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.components().count() > 1 {
        return executable_file(path).then(|| path.to_path_buf());
    }
    let search_path = std::env::var_os("PATH")?;
    let extensions = executable_extensions();
    for directory in std::env::split_paths(&search_path) {
        for extension in &extensions {
            let candidate = directory.join(format!("{name}{extension}"));
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn executable_extensions() -> Vec<String> {
    if cfg!(windows) {
        let mut extensions = std::env::var("PATHEXT")
            .ok()
            .map(|value| {
                value
                    .split(';')
                    .filter(|item| !item.is_empty())
                    .map(|item| item.to_ascii_lowercase())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![".exe".to_owned(), ".cmd".to_owned(), ".bat".to_owned()]);
        extensions.push(String::new());
        extensions
    } else {
        vec![String::new()]
    }
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_have_unique_ids() {
        let mut ids = std::collections::HashSet::new();
        for definition in tool_definitions() {
            assert!(ids.insert(definition.id), "duplicate {}", definition.id);
        }
    }
}
