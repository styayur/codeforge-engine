use std::path::Path;

use codeforge_protocol::Language;
use tree_sitter::{Language as TsLanguage, Parser, Tree};

#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("tree-sitter grammar is unavailable for {0}")]
    Unsupported(Language),
    #[error(transparent)]
    Language(#[from] tree_sitter::LanguageError),
}

pub fn parser_for(language: Language, path: Option<&Path>) -> Result<Parser, ParserError> {
    let mut parser = Parser::new();
    let grammar = grammar_for(language, path).ok_or(ParserError::Unsupported(language))?;
    parser.set_language(&grammar)?;
    Ok(parser)
}

pub fn grammar_for(language: Language, path: Option<&Path>) -> Option<TsLanguage> {
    Some(match language {
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        Language::C => tree_sitter_c::LANGUAGE.into(),
        Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        Language::TypeScript => {
            if path
                .and_then(Path::extension)
                .and_then(|extension| extension.to_str())
                == Some("tsx")
            {
                tree_sitter_typescript::LANGUAGE_TSX.into()
            } else {
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
            }
        }
        Language::Java => tree_sitter_java::LANGUAGE.into(),
        Language::Go => tree_sitter_go::LANGUAGE.into(),
        Language::Dart => tree_sitter_dart::LANGUAGE.into(),
        Language::PowerShell => tree_sitter_powershell::LANGUAGE.into(),
        Language::Markdown
        | Language::Json
        | Language::Yaml
        | Language::Toml
        | Language::Html
        | Language::Css => return None,
    })
}

pub fn parse(language: Language, path: &Path, source: &str) -> Result<Tree, ParserError> {
    let mut parser = parser_for(language, Some(path))?;
    Ok(parser
        .parse(source, None)
        .expect("tree-sitter parse returns a tree for valid UTF-8 input"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dart_grammar_parses_real_source_and_preserves_utf8_ranges() {
        let source = "// 自有合成样例\nclass Example {\n  String? name;\n  Future<List<int>> values() async {\n    return [1, 2, 3];\n  }\n}\n";
        let tree = parse(Language::Dart, Path::new("example.dart"), source).expect("Dart grammar");
        let root = tree.root_node();
        assert!(!root.has_error(), "{}", root.to_sexp());
        assert_eq!(root.end_byte(), source.len());
        let mut cursor = root.walk();
        let class = root
            .named_children(&mut cursor)
            .find(|node| node.kind() == "class_declaration")
            .expect("class AST, not a registry-only check");
        assert_eq!(class.start_position().row, 1);
        assert!(
            class
                .utf8_text(source.as_bytes())
                .expect("class node spans valid UTF-8 source")
                .contains("Future<List<int>>")
        );
    }

    #[test]
    fn dart_grammar_reports_malformed_source_and_recovers_for_next_parse() {
        let mut parser =
            parser_for(Language::Dart, Some(Path::new("example.dart"))).expect("Dart grammar");
        let malformed = parser
            .parse("void main( {", None)
            .expect("parser returns an error tree for malformed UTF-8 source text");
        assert!(malformed.root_node().has_error());
        let valid = parser
            .parse("void main() {}", None)
            .expect("parser returns a tree after malformed input");
        assert!(!valid.root_node().has_error());
    }
}
