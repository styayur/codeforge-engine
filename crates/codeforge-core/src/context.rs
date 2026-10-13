//! Read-only, explicitly requested source context and heuristic impact suggestions.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};

use codeforge_engines::syntax_facts;
use codeforge_protocol::SourceRange;
use codeforge_workspace::{WorkspaceManager, hash_bytes};
use serde::Serialize;

use crate::CoreError;

const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_EDGES: usize = 20_000;

#[derive(Debug, Clone, Serialize)]
pub struct ContextSymbol {
    pub id: usize,
    pub file: PathBuf,
    pub name: String,
    pub range: SourceRange,
    pub content_hash: String,
    pub test_hint: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Relation {
    pub caller: usize,
    pub candidate_callee: usize,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SourceGraph {
    pub symbols: Vec<ContextSymbol>,
    pub relations: Vec<Relation>,
    pub unresolved_calls: usize,
    pub partial: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceFragment {
    pub symbol: ContextSymbol,
    pub text: String,
    pub excerpt_range: SourceRange,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextReport {
    pub evidence: &'static str,
    pub fragments: Vec<SourceFragment>,
    pub partial: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub evidence: &'static str,
    pub changed_files: Vec<PathBuf>,
    pub impacted_symbols: Vec<ContextSymbol>,
    pub related_tests: Vec<ContextSymbol>,
    pub partial: bool,
    pub limitations: &'static str,
}

impl SourceGraph {
    /// No process, network or persistent index. Workspace ignore rules are reused.
    pub fn build(root: &Path, max_files: usize) -> Result<Self, CoreError> {
        let workspace = WorkspaceManager::open(root)?;
        let mut records = workspace.records();
        records.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        let limit = max_files.clamp(1, 8192);
        let mut graph = Self {
            partial: records.len() > limit,
            ..Self::default()
        };
        let mut calls = Vec::new();
        for record in records.into_iter().take(limit) {
            if graph.symbols.len() >= 20_000 {
                graph.partial = true;
                break;
            }
            let Some(language) = record.language else {
                continue;
            };
            let source = match read_source(workspace.root(), &record.relative_path) {
                Ok(source) => source,
                Err(_) => {
                    graph.partial = true;
                    graph.warnings.push(format!(
                        "unavailable or outside bounds: {}",
                        record.relative_path.display()
                    ));
                    continue;
                }
            };
            let facts = match syntax_facts(language, &record.relative_path, &source) {
                Ok(facts) => facts,
                Err(_) => {
                    graph.partial = true;
                    continue;
                }
            };
            graph.partial |= facts.truncated || facts.syntax_errors;
            let content_hash = hash_bytes(source.as_bytes());
            for symbol in facts.symbols {
                if graph.symbols.len() >= 20_000 {
                    graph.partial = true;
                    break;
                }
                let id = graph.symbols.len();
                calls.push((id, symbol.calls));
                let path = record.relative_path.to_string_lossy().replace('\\', "/");
                graph.symbols.push(ContextSymbol {
                    id,
                    file: record.relative_path.clone(),
                    name: symbol.name,
                    range: symbol.range,
                    content_hash: content_hash.clone(),
                    test_hint: symbol.test_hint
                        || path.starts_with("tests/")
                        || path.contains("/tests/")
                        || path.contains("test_"),
                });
            }
        }
        let mut names: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for symbol in &graph.symbols {
            names.entry(&symbol.name).or_default().push(symbol.id);
        }
        'calls: for (caller, calls) in calls {
            for name in calls {
                let Some(targets) = names.get(name.as_str()) else {
                    graph.unresolved_calls += 1;
                    continue;
                };
                for &candidate_callee in targets {
                    if graph.relations.len() >= MAX_EDGES {
                        graph.partial = true;
                        break 'calls;
                    }
                    graph.relations.push(Relation {
                        caller,
                        candidate_callee,
                        ambiguous: targets.len() > 1,
                    });
                }
            }
        }
        Ok(graph)
    }

    /// Literal symbol-name search with exact excerpts, not semantic relevance scores.
    pub fn extract(
        &self,
        root: &Path,
        query: &str,
        limit: usize,
        byte_budget: usize,
    ) -> Result<ContextReport, CoreError> {
        let root = root.canonicalize()?;
        let mut fragments = Vec::new();
        let mut remaining = byte_budget.min(256 * 1024);
        let mut partial = self.partial;
        let mut warnings = self.warnings.clone();
        let limit = limit.min(100);
        for symbol in self
            .symbols
            .iter()
            .filter(|symbol| symbol.name.contains(query))
        {
            if remaining == 0 {
                partial = true;
                break;
            }
            if fragments.len() >= limit {
                partial = true;
                break;
            }
            let source = match read_source(&root, &symbol.file) {
                Ok(source) if hash_bytes(source.as_bytes()) == symbol.content_hash => source,
                _ => {
                    partial = true;
                    warnings.push(format!(
                        "source changed or unavailable: {}",
                        symbol.file.display()
                    ));
                    continue;
                }
            };
            let start = symbol.range.start_byte;
            let end = symbol.range.end_byte;
            let mut excerpt_end = end.min(start.saturating_add(remaining));
            while !source.is_char_boundary(excerpt_end) {
                excerpt_end -= 1;
            }
            if excerpt_end == start {
                partial = true;
                break;
            }
            let text = source[start..excerpt_end].to_owned();
            remaining -= text.len();
            let truncated = excerpt_end != end;
            partial |= truncated;
            fragments.push(SourceFragment {
                symbol: symbol.clone(),
                text,
                truncated,
                excerpt_range: SourceRange::from_offsets(&source, start, excerpt_end)
                    .map_err(|error| CoreError::InvalidProject(error.to_string()))?,
            });
        }
        Ok(ContextReport {
            evidence: "syntax-extraction",
            fragments,
            partial,
            warnings,
        })
    }

    /// Reverse reachability across name-based candidate edges; never a test gate.
    pub fn impact(&self, changed_files: Vec<PathBuf>, max_depth: usize) -> ImpactReport {
        let changed: BTreeSet<_> = changed_files.iter().collect();
        let mut seen: BTreeSet<_> = self
            .symbols
            .iter()
            .filter(|s| changed.contains(&s.file))
            .map(|s| s.id)
            .collect();
        let mut queue: VecDeque<_> = seen.iter().map(|&id| (id, 0)).collect();
        let mut reverse: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for edge in &self.relations {
            reverse
                .entry(edge.candidate_callee)
                .or_default()
                .push(edge.caller);
        }
        let mut partial = self.partial
            || changed_files
                .iter()
                .any(|file| !self.symbols.iter().any(|symbol| &symbol.file == file));
        while let Some((id, depth)) = queue.pop_front() {
            for &caller in reverse.get(&id).into_iter().flatten() {
                if seen.contains(&caller) {
                    continue;
                }
                if depth >= max_depth.min(32) {
                    partial = true;
                    continue;
                }
                seen.insert(caller);
                queue.push_back((caller, depth + 1));
            }
        }
        let impacted_symbols: Vec<_> = seen
            .into_iter()
            .map(|id| self.symbols[id].clone())
            .collect();
        let related_tests = impacted_symbols
            .iter()
            .filter(|s| s.test_hint)
            .cloned()
            .collect();
        ImpactReport {
            evidence: "heuristic",
            changed_files,
            impacted_symbols,
            related_tests,
            partial,
            limitations: "Name-based candidate edges may be ambiguous or missing. Imports, dynamic dispatch, macros, deleted symbols and external callers are not resolved. Suggested tests never replace the full verification gate.",
        }
    }

    pub async fn impact_changed(
        &self,
        root: &Path,
        max_depth: usize,
    ) -> Result<ImpactReport, CoreError> {
        let repository = codeforge_git::GitRepository::discover(root)?;
        let root = root.canonicalize()?;
        let repository_root = repository.root().canonicalize()?;
        let mut changed = Vec::new();
        for file in repository.changed_files().await? {
            let full = repository_root.join(file.path);
            if let Ok(relative) = full.strip_prefix(&root) {
                changed.push(relative.to_path_buf());
            }
        }
        changed.sort();
        changed.dedup();
        Ok(self.impact(changed, max_depth))
    }
}

fn read_source(root: &Path, relative: &Path) -> Result<String, CoreError> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(CoreError::InvalidProject(
            "context path must be repository-relative".into(),
        ));
    }
    let path = root.join(relative).canonicalize()?;
    if !path.starts_with(root) {
        return Err(CoreError::InvalidProject(
            "context path escapes repository".into(),
        ));
    }
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(CoreError::InvalidProject(
            "context file exceeds byte limit".into(),
        ));
    }
    String::from_utf8(bytes).map_err(|error| CoreError::InvalidProject(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_context_and_impact_are_bounded_deterministic_and_read_only() {
        let dir = tempfile::tempdir().expect("fixture");
        std::fs::write(dir.path().join("a.rs"), "pub fn leaf() {}\npub fn caller() { leaf(); }\n#[test]\nfn test_caller() { caller(); }\n").expect("fixture");
        let graph = SourceGraph::build(dir.path(), 10).expect("fixture");
        assert_eq!(graph.relations.len(), 2);
        let impact = graph.impact(vec![PathBuf::from("a.rs")], 4);
        assert_eq!(impact.related_tests.len(), 1);
        let context = graph.extract(dir.path(), "caller", 1, 20).expect("fixture");
        assert_eq!(context.fragments.len(), 1);
        assert!(context.partial);
        assert_eq!(context.fragments[0].excerpt_range.start_line, 2);
        assert_eq!(std::fs::read_dir(dir.path()).expect("fixture").count(), 1);
        let again = SourceGraph::build(dir.path(), 10).expect("fixture");
        assert_eq!(
            serde_json::to_string(&graph).expect("fixture"),
            serde_json::to_string(&again).expect("fixture")
        );
        std::fs::write(dir.path().join("a.rs"), "fn replaced() {}\n").expect("fixture");
        assert!(
            graph
                .extract(dir.path(), "caller", 10, 100)
                .expect("fixture")
                .fragments
                .is_empty()
        );
    }

    #[test]
    fn ambiguity_cycles_and_depth_limits_are_explicit() {
        let dir = tempfile::tempdir().expect("fixture");
        std::fs::write(dir.path().join("a.rs"), "fn target() { caller(); }").expect("fixture");
        std::fs::write(
            dir.path().join("b.rs"),
            "fn target() {}\nfn caller() { target(); }",
        )
        .expect("fixture");
        let graph = SourceGraph::build(dir.path(), 10).expect("fixture");
        assert!(graph.relations.iter().any(|e| e.ambiguous));
        assert!(graph.impact(vec!["a.rs".into()], 0).partial);
        assert_eq!(
            graph.impact(vec!["a.rs".into()], 10).impacted_symbols.len(),
            2
        );
        let root = dir.path().canonicalize().expect("fixture");
        assert!(read_source(&root, Path::new("../outside.rs")).is_err());
    }

    #[test]
    fn unicode_budgets_and_missing_changed_files_are_reported() {
        let dir = tempfile::tempdir().expect("fixture");
        std::fs::write(dir.path().join("a.py"), "def 年度():\n    return '报告'\n")
            .expect("fixture");
        let graph = SourceGraph::build(dir.path(), 10).expect("fixture");
        let context = graph.extract(dir.path(), "年度", 10, 6).expect("fixture");
        assert!(context.partial);
        assert!(
            context
                .fragments
                .iter()
                .all(|fragment| fragment.text.len() <= 6)
        );
        assert!(graph.impact(vec!["deleted.py".into()], 4).partial);
        assert!(
            graph
                .extract(dir.path(), "", 0, 100)
                .expect("fixture")
                .fragments
                .is_empty()
        );
    }

    #[tokio::test]
    async fn git_changes_use_the_same_canonical_root_on_windows() {
        let dir = tempfile::tempdir().expect("fixture");
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(dir.path())
                .status()
                .expect("fixture")
                .success()
        );
        std::fs::write(dir.path().join("a.rs"), "fn changed() {}\n").expect("fixture");
        let graph = SourceGraph::build(dir.path(), 10).expect("fixture");
        let report = graph.impact_changed(dir.path(), 4).await.expect("fixture");
        assert_eq!(report.changed_files, [PathBuf::from("a.rs")]);
        assert_eq!(report.impacted_symbols.len(), 1);
    }
}
