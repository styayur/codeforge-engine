//! Bounded syntax facts for on-demand context. Name matching is not call resolution.
use std::path::Path;

use codeforge_protocol::{Language, SourceRange};
use serde::Serialize;

use crate::{EngineError, language};

#[derive(Debug, Clone, Serialize)]
pub struct SyntaxSymbol {
    pub name: String,
    pub range: SourceRange,
    pub calls: Vec<String>,
    pub test_hint: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SyntaxFacts {
    pub symbols: Vec<SyntaxSymbol>,
    pub truncated: bool,
    pub syntax_errors: bool,
}

/// Reuse the existing grammars. Bounds limit AST work as well as returned facts.
pub fn syntax_facts(
    language: Language,
    path: &Path,
    source: &str,
) -> Result<SyntaxFacts, EngineError> {
    let tree = language::parse(language, path, source)
        .map_err(|error| EngineError::Failed(error.to_string()))?;
    let mut facts = SyntaxFacts {
        syntax_errors: tree.root_node().has_error(),
        ..SyntaxFacts::default()
    };
    let mut stack = vec![(tree.root_node(), None)];
    let mut visited = 0;
    let mut calls = 0;
    while let Some((node, mut owner)) = stack.pop() {
        visited += 1;
        if visited > 100_000 || facts.symbols.len() >= 512 || calls >= 4096 {
            facts.truncated = true;
            break;
        }
        if matches!(
            node.kind(),
            "function_item"
                | "function_definition"
                | "function_declaration"
                | "method_definition"
                | "method_declaration"
        ) && let Some(name) = node.child_by_field_name("name")
            && let Ok(name) = name.utf8_text(source.as_bytes())
        {
            let range = SourceRange::from_offsets(source, node.start_byte(), node.end_byte())
                .map_err(|error| EngineError::Failed(error.to_string()))?;
            owner = Some(facts.symbols.len());
            let prefix = &source[..node.start_byte()];
            let preceding = prefix.lines().rev().take(2).collect::<Vec<_>>().join(" ");
            facts.symbols.push(SyntaxSymbol {
                name: name.to_owned(),
                range,
                calls: Vec::new(),
                test_hint: name.starts_with("test_")
                    || name.starts_with("Test")
                    || preceding.contains("#[test]")
                    || preceding.contains("#[tokio::test]"),
            });
        }
        if matches!(
            node.kind(),
            "call_expression" | "call" | "method_invocation"
        ) && let Some(owner) = owner
            && let Some(function) = node
                .child_by_field_name("function")
                .or_else(|| node.child_by_field_name("name"))
        {
            let name = if matches!(function.kind(), "identifier" | "field_identifier") {
                Some(function)
            } else {
                function
                    .child_by_field_name("name")
                    .or_else(|| function.child_by_field_name("field"))
                    .or_else(|| function.child_by_field_name("attribute"))
            };
            if let Some(name) = name.and_then(|name| name.utf8_text(source.as_bytes()).ok()) {
                facts.symbols[owner].calls.push(name.to_owned());
                calls += 1;
            }
        }
        let mut cursor = node.walk();
        let children: Vec<_> = node.named_children(&mut cursor).collect();
        stack.extend(children.into_iter().rev().map(|child| (child, owner)));
    }
    for symbol in &mut facts.symbols {
        symbol.calls.sort();
        symbol.calls.dedup();
    }
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_calls_but_not_comments_or_string_literals() {
        let source = "fn leaf() {}\n#[test]\nfn test_leaf() { leaf(); let s = \"imaginary()\"; /* nope() */ }";
        let facts = syntax_facts(Language::Rust, Path::new("test.rs"), source).unwrap();
        assert_eq!(facts.symbols.len(), 2);
        assert_eq!(facts.symbols[1].calls, ["leaf"]);
        assert!(facts.symbols[1].test_hint);
        assert_eq!(facts.symbols[1].range.start_line, 3);
    }

    #[test]
    fn nested_python_calls_belong_to_the_inner_function() {
        let facts = syntax_facts(
            Language::Python,
            Path::new("a.py"),
            "def outer():\n    def inner():\n        target()\n    inner()\n",
        )
        .unwrap();
        assert_eq!(facts.symbols[0].calls, ["inner"]);
        assert_eq!(facts.symbols[1].calls, ["target"]);
    }
}
