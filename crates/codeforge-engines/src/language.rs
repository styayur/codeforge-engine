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
