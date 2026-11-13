use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use codeforge_protocol::{Confidence, DetectedProjectProfile, Language, ProjectProfile};
use walkdir::WalkDir;

use crate::FleetError;

#[derive(Debug, Clone)]
pub struct DetectedWorkspace {
    pub profile: ProjectProfile,
    pub tool_hints: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectDetector;

impl ProjectDetector {
    pub fn detect(root: &Path) -> Result<DetectedWorkspace, FleetError> {
        if !root.is_dir() {
            return Err(FleetError::MissingRepository(root.to_path_buf()));
        }
        let files = collect_files(root)?;
        let mut ecosystems = Vec::new();
        let mut tool_hints = Vec::new();

        if root.join("Cargo.toml").exists() {
            ecosystems.push(project(
                "Rust workspace",
                &[Language::Rust],
                &["Cargo.toml"],
                Confidence::High,
                [
                    ("format", vec!["cargo", "fmt", "--all", "--", "--check"]),
                    (
                        "lint",
                        vec![
                            "cargo",
                            "clippy",
                            "--workspace",
                            "--all-targets",
                            "--all-features",
                            "--",
                            "-D",
                            "warnings",
                        ],
                    ),
                    ("test", vec!["cargo", "test", "--workspace"]),
                    ("build", vec!["cargo", "build", "--workspace"]),
                    ("benchmark", vec!["cargo", "bench"]),
                ],
            ));
            tool_hints.extend(["rustfmt", "clippy"].map(str::to_owned));
        }

        if root.join("package.json").exists() || root.join("tsconfig.json").exists() {
            let mut languages = vec![Language::JavaScript];
            if root.join("tsconfig.json").exists()
                || files.iter().any(|file| {
                    matches!(
                        file.extension().and_then(|extension| extension.to_str()),
                        Some("ts" | "tsx" | "mts" | "cts")
                    )
                })
            {
                languages.push(Language::TypeScript);
            }
            let manager = package_manager(root);
            let mut commands = BTreeMap::new();
            commands.insert(
                "lint".to_owned(),
                vec![manager.clone(), "run".to_owned(), "lint".to_owned()],
            );
            commands.insert("test".to_owned(), vec![manager.clone(), "test".to_owned()]);
            commands.insert(
                "build".to_owned(),
                vec![manager.clone(), "run".to_owned(), "build".to_owned()],
            );
            commands.insert(
                "benchmark".to_owned(),
                vec![manager, "run".to_owned(), "bench".to_owned()],
            );
            ecosystems.push(DetectedProjectProfile {
                ecosystem: if languages.contains(&Language::TypeScript) {
                    "Node + TypeScript".to_owned()
                } else {
                    "Node".to_owned()
                },
                languages,
                markers: ["package.json", "tsconfig.json"]
                    .into_iter()
                    .filter(|marker| root.join(marker).exists())
                    .map(str::to_owned)
                    .collect(),
                confidence: Confidence::High,
                suggested_commands: commands,
            });
            tool_hints.extend(["biome", "prettier", "eslint", "oxlint"].map(str::to_owned));
        }

        if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
            ecosystems.push(project(
                "Python",
                &[Language::Python],
                &["pyproject.toml", "requirements.txt"],
                Confidence::High,
                [
                    ("format", vec!["ruff", "format", "--check", "."]),
                    ("lint", vec!["ruff", "check", "."]),
                    ("test", vec!["pytest"]),
                ],
            ));
            tool_hints.extend(["ruff", "pytest"].map(str::to_owned));
        }

        if root.join("go.mod").exists() {
            ecosystems.push(project(
                "Go module",
                &[Language::Go],
                &["go.mod"],
                Confidence::High,
                [
                    ("format", vec!["gofmt", "-l", "."]),
                    ("lint", vec!["staticcheck", "./..."]),
                    ("test", vec!["go", "test", "./..."]),
                    ("build", vec!["go", "build", "./..."]),
                    (
                        "benchmark",
                        vec!["go", "test", "-bench=.", "-run=^$", "./..."],
                    ),
                ],
            ));
            tool_hints.extend(["gofmt", "gofumpt", "staticcheck"].map(str::to_owned));
        }

        if root.join("pom.xml").exists()
            || root.join("build.gradle").exists()
            || root.join("build.gradle.kts").exists()
        {
            let (program, test_args, build_args) = if root.join("pom.xml").exists() {
                ("mvn", vec!["test"], vec!["verify"])
            } else {
                ("gradle", vec!["test"], vec!["build"])
            };
            ecosystems.push(project(
                "Java build",
                &[Language::Java],
                &["pom.xml", "build.gradle", "build.gradle.kts"],
                Confidence::High,
                [
                    ("test", std::iter::once(program).chain(test_args).collect()),
                    (
                        "build",
                        std::iter::once(program).chain(build_args).collect(),
                    ),
                ],
            ));
            tool_hints
                .extend(["google-java-format", "spotless", "mvn", "gradle"].map(str::to_owned));
        }

        if root.join("CMakeLists.txt").exists() || files.iter().any(|file| is_c_file(file)) {
            ecosystems.push(project(
                "C / C++",
                &[Language::C],
                &["CMakeLists.txt"],
                Confidence::Medium,
                [("build", vec!["cmake", "--build", "build"])],
            ));
            tool_hints.extend(["clang-format", "clang-tidy", "cmake"].map(str::to_owned));
        }

        if root.join("pubspec.yaml").exists() {
            ecosystems.push(project(
                "Flutter / Dart",
                &[Language::Dart],
                &["pubspec.yaml"],
                Confidence::High,
                [
                    (
                        "format",
                        vec![
                            "dart",
                            "format",
                            "--output=none",
                            "--set-exit-if-changed",
                            ".",
                        ],
                    ),
                    ("lint", vec!["dart", "analyze"]),
                    ("test", vec!["flutter", "test"]),
                ],
            ));
            tool_hints.extend(["dart", "flutter"].map(str::to_owned));
        }

        let powershell_markers = files
            .iter()
            .filter_map(|file| file.file_name().and_then(|name| name.to_str()))
            .filter(|name| {
                name.ends_with(".ps1") || name.ends_with(".psm1") || name.ends_with(".psd1")
            })
            .take(8)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if !powershell_markers.is_empty() {
            ecosystems.push(DetectedProjectProfile {
                ecosystem: "PowerShell".to_owned(),
                languages: vec![Language::PowerShell],
                markers: powershell_markers,
                confidence: Confidence::High,
                suggested_commands: BTreeMap::from([(
                    "lint".to_owned(),
                    vec![
                        "pwsh".to_owned(),
                        "-NoProfile".to_owned(),
                        "-NonInteractive".to_owned(),
                        "-Command".to_owned(),
                        "Invoke-ScriptAnalyzer -Path . -Recurse -Severity Warning,Error".to_owned(),
                    ],
                )]),
            });
            tool_hints.push("pwsh".to_owned());
        }

        let doc_languages = collect_marker_languages(root, &files);
        if !doc_languages.is_empty() {
            ecosystems.push(DetectedProjectProfile {
                ecosystem: "Documentation and configuration".to_owned(),
                languages: doc_languages,
                markers: collect_doc_markers(&files),
                confidence: Confidence::Medium,
                suggested_commands: BTreeMap::from([(
                    "format".to_owned(),
                    vec!["prettier".to_owned(), "--check".to_owned(), ".".to_owned()],
                )]),
            });
            tool_hints.extend(["prettier", "biome", "markdownlint"].map(str::to_owned));
        }

        if ecosystems.is_empty() {
            ecosystems.push(DetectedProjectProfile {
                ecosystem: "Unknown / mixed workspace".to_owned(),
                languages: Vec::new(),
                markers: Vec::new(),
                confidence: Confidence::Low,
                suggested_commands: BTreeMap::new(),
            });
        }

        tool_hints.sort();
        tool_hints.dedup();
        let mut languages = ecosystems
            .iter()
            .flat_map(|profile| profile.languages.iter().copied())
            .collect::<Vec<_>>();
        languages.sort();
        languages.dedup();
        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repository")
            .to_owned();
        Ok(DetectedWorkspace {
            profile: ProjectProfile {
                root: root.to_path_buf(),
                name,
                ecosystems,
                languages,
            },
            tool_hints,
        })
    }
}

pub fn suggest_codeforge_toml(profile: &ProjectProfile) -> String {
    let mut output =
        String::from("# Suggested by CodeForge v0.2 project detection. Review before using.\n");
    for ecosystem in &profile.ecosystems {
        if ecosystem.suggested_commands.is_empty() {
            continue;
        }
        output.push_str(&format!("\n# {}\n", ecosystem.ecosystem));
        for (name, command) in &ecosystem.suggested_commands {
            output.push_str(&format!(
                "\n[commands.{name}]\nprogram = {:?}\nargs = [",
                command[0]
            ));
            output.push_str(
                &command
                    .iter()
                    .skip(1)
                    .map(|arg| format!("{arg:?}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            output.push_str("]\n");
        }
    }
    output
}

fn project<const N: usize, const M: usize>(
    ecosystem: &str,
    languages: &[Language],
    markers: &[&str; M],
    confidence: Confidence,
    commands: [(&str, Vec<&str>); N],
) -> DetectedProjectProfile {
    DetectedProjectProfile {
        ecosystem: ecosystem.to_owned(),
        languages: languages.to_vec(),
        markers: markers.iter().map(|marker| (*marker).to_owned()).collect(),
        confidence,
        suggested_commands: commands
            .into_iter()
            .map(|(name, args)| {
                (
                    name.to_owned(),
                    args.into_iter().map(str::to_owned).collect(),
                )
            })
            .collect(),
    }
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>, FleetError> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!(%error, root = %root.display(), "skipping unreadable detection entry");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(root).unwrap_or(entry.path());
        if relative.components().any(|component| {
            matches!(
                component.as_os_str().to_string_lossy().as_ref(),
                ".git" | ".codeforge" | "target" | "node_modules" | "dist" | "build" | "vendor"
            )
        }) {
            continue;
        }
        files.push(entry.path().to_path_buf());
    }
    Ok(files)
}

fn package_manager(root: &Path) -> String {
    if root.join("pnpm-lock.yaml").exists() {
        "pnpm".to_owned()
    } else if root.join("yarn.lock").exists() {
        "yarn".to_owned()
    } else {
        "npm".to_owned()
    }
}

fn is_c_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("c" | "h" | "cc" | "cpp" | "hpp")
    )
}

fn collect_marker_languages(root: &Path, files: &[PathBuf]) -> Vec<Language> {
    let mut languages = Vec::new();
    for (language, markers) in [
        (Language::Markdown, &["README.md", "README.markdown"][..]),
        (Language::Json, &["package.json", "tsconfig.json"][..]),
        (Language::Yaml, &["pubspec.yaml"][..]),
        (Language::Toml, &["Cargo.toml", "pyproject.toml"][..]),
    ] {
        if markers.iter().any(|marker| root.join(marker).exists())
            || files.iter().any(|file| {
                file.extension()
                    .and_then(|extension| extension.to_str())
                    .and_then(Language::from_extension)
                    == Some(language)
            })
        {
            languages.push(language);
        }
    }
    if files.iter().any(|file| {
        matches!(
            file.extension().and_then(|extension| extension.to_str()),
            Some("html" | "htm")
        )
    }) {
        languages.push(Language::Html);
    }
    if files.iter().any(|file| {
        matches!(
            file.extension().and_then(|extension| extension.to_str()),
            Some("css" | "scss")
        )
    }) {
        languages.push(Language::Css);
    }
    languages.sort();
    languages.dedup();
    languages
}

fn collect_doc_markers(files: &[PathBuf]) -> Vec<String> {
    files
        .iter()
        .filter(|file| {
            file.extension()
                .and_then(|extension| extension.to_str())
                .and_then(Language::from_extension)
                .is_some_and(|language| {
                    matches!(
                        language,
                        Language::Markdown
                            | Language::Json
                            | Language::Yaml
                            | Language::Toml
                            | Language::Html
                            | Language::Css
                    )
                })
        })
        .filter_map(|file| file.file_name().and_then(|name| name.to_str()))
        .take(12)
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_and_docs() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("Cargo.toml"), "[workspace]\n").expect("cargo");
        std::fs::write(temp.path().join("README.md"), "# test\n").expect("readme");
        let detected = ProjectDetector::detect(temp.path()).expect("detect");
        assert!(detected.profile.languages.contains(&Language::Rust));
        assert!(detected.profile.languages.contains(&Language::Markdown));
    }

    #[test]
    fn generates_reviewable_config_plan() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("pubspec.yaml"), "name: test\n").expect("pubspec");
        let detected = ProjectDetector::detect(temp.path()).expect("detect");
        let config = suggest_codeforge_toml(&detected.profile);
        assert!(config.contains("dart"));
        assert!(!config.contains("shell"));
    }
}
