use std::path::Path;

use codeforge_protocol::{
    Confidence, Diagnostic, DiagnosticCategory, Fix, Language, RiskLevel, Severity, SourceRange,
    TextEdit, Transformation,
};
use tree_sitter::Node;

use crate::language;

#[derive(Debug, Clone)]
struct FixSpec {
    replacement: String,
    safe: bool,
    title: String,
}

#[derive(Debug, Clone)]
struct Finding {
    rule_id: String,
    severity: Severity,
    category: DiagnosticCategory,
    confidence: Confidence,
    message: String,
    explanation: String,
    start_byte: usize,
    end_byte: usize,
    fix: Option<FixSpec>,
}

pub fn analyze_source(
    engine: &str,
    language_id: Language,
    path: &Path,
    source: &str,
) -> Vec<Diagnostic> {
    let tree = match language::parse(language_id, path, source) {
        Ok(tree) => tree,
        Err(error) => {
            let range = SourceRange::from_offsets(source, 0, source.len().min(1))
                .unwrap_or(SourceRange::new(0, 0, 1, 1, 1, 1).expect("empty range"));
            return vec![Diagnostic::new(
                engine,
                language_id,
                "PARSE-000",
                Severity::Error,
                DiagnosticCategory::Correctness,
                Confidence::High,
                path.to_path_buf(),
                range,
                format!("Tree-sitter grammar could not be loaded: {error}"),
            )];
        }
    };

    let mut findings = Vec::new();
    collect_tree_findings(language_id, source, tree.root_node(), &mut findings);
    findings
        .into_iter()
        .take(250)
        .map(|finding| finding_to_diagnostic(engine, language_id, path, source, finding))
        .collect()
}

pub fn transformations_from_source(
    engine: &str,
    language_id: Language,
    path: &Path,
    source: &str,
) -> Vec<Transformation> {
    analyze_source(engine, language_id, path, source)
        .into_iter()
        .flat_map(|diagnostic| {
            let file = diagnostic.file.clone();
            diagnostic.fixes.into_iter().map(move |fix| {
                let mut transformation = Transformation::new(
                    diagnostic.engine.clone(),
                    diagnostic.language,
                    fix.title,
                    diagnostic.message.clone(),
                    vec![codeforge_protocol::FileEdit {
                        file: file.clone(),
                        edits: fix.edits,
                    }],
                );
                transformation.risk_level = if fix.safe {
                    RiskLevel::Low
                } else {
                    RiskLevel::Medium
                };
                transformation
            })
        })
        .collect()
}

fn finding_to_diagnostic(
    engine: &str,
    language: Language,
    path: &Path,
    source: &str,
    finding: Finding,
) -> Diagnostic {
    let range = SourceRange::from_offsets(source, finding.start_byte, finding.end_byte)
        .unwrap_or_else(|_| SourceRange::new(0, 0, 1, 1, 1, 1).expect("empty range"));
    let mut diagnostic = Diagnostic::new(
        engine,
        language,
        finding.rule_id,
        finding.severity,
        finding.category,
        finding.confidence,
        path.to_path_buf(),
        range.clone(),
        finding.message,
    )
    .with_explanation(finding.explanation);
    if let Some(fix) = finding.fix {
        diagnostic = diagnostic.with_fix(Fix::new(
            fix.title,
            vec![TextEdit {
                range,
                replacement: fix.replacement,
                description: None,
            }],
            fix.safe,
        ));
    }
    diagnostic
}

fn collect_tree_findings(
    language: Language,
    source: &str,
    node: Node<'_>,
    findings: &mut Vec<Finding>,
) {
    if node.is_error() || node.is_missing() {
        let start = node.start_byte();
        let end = node
            .end_byte()
            .max(start.saturating_add(1))
            .min(source.len());
        findings.push(Finding {
            rule_id: "PARSE-001".to_owned(),
            severity: Severity::Error,
            category: DiagnosticCategory::Correctness,
            confidence: Confidence::High,
            message: if node.is_missing() {
                format!("Missing syntax element: {}", node.kind())
            } else {
                "Syntax error".to_owned()
            },
            explanation: "Tree-sitter could not parse this region. Later findings in the same subtree may be incomplete.".to_owned(),
            start_byte: start,
            end_byte: end,
            fix: None,
        });
    }

    if let Some(finding) = rule_for_node(language, source, node) {
        findings.push(finding);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_tree_findings(language, source, child, findings);
    }
}

fn rule_for_node(language: Language, source: &str, node: Node<'_>) -> Option<Finding> {
    match language {
        Language::Python => python_rule(source, node),
        Language::Rust => rust_rule(source, node),
        Language::C => c_rule(source, node),
        Language::JavaScript | Language::TypeScript => javascript_rule(source, node),
        Language::Java => java_rule(source, node),
        Language::Go => go_rule(source, node),
    }
}

fn python_rule(source: &str, node: Node<'_>) -> Option<Finding> {
    let text = node_text(source, node);
    match node.kind() {
        "except_clause" if text.trim() == "except:" => Some(Finding {
            rule_id: "PY-STYLE-001".to_owned(),
            severity: Severity::Warning,
            category: DiagnosticCategory::Correctness,
            confidence: Confidence::High,
            message: "Bare except catches system-exiting exceptions".to_owned(),
            explanation: "Use `except Exception:` unless BaseException handling is intentional."
                .to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: Some(FixSpec {
                replacement: "except Exception:".to_owned(),
                safe: true,
                title: "Replace bare except".to_owned(),
            }),
        }),
        "comparison_operator"
            if text.contains("None") && (text.contains("==") || text.contains("!=")) =>
        {
            let (operator, replacement_operator) = if text.contains("!=") {
                ("!=", " is not ")
            } else {
                ("==", " is ")
            };
            let (left, right) = text.split_once(operator)?;
            let replacement = format!("{}{}{}", left.trim(), replacement_operator, right.trim());
            Some(Finding {
                rule_id: "PY-CORRECTNESS-001".to_owned(),
                severity: Severity::Warning,
                category: DiagnosticCategory::Correctness,
                confidence: Confidence::High,
                message: "Identity should be used when comparing with None".to_owned(),
                explanation: "`== None` can be overridden by user-defined equality and is less precise than `is None`.".to_owned(),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                fix: Some(FixSpec {
                    replacement,
                    safe: true,
                    title: "Use identity comparison".to_owned(),
                }),
            })
        }
        "call" if text.starts_with("print(") => Some(Finding {
            rule_id: "PY-MAINTAINABILITY-001".to_owned(),
            severity: Severity::Info,
            category: DiagnosticCategory::Maintainability,
            confidence: Confidence::Low,
            message: "Print call may be debug output".to_owned(),
            explanation: "Library code usually uses logging or structured output instead of print."
                .to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        _ => None,
    }
}

fn rust_rule(source: &str, node: Node<'_>) -> Option<Finding> {
    let text = node_text(source, node);
    match node.kind() {
        "call_expression" if text.ends_with(".unwrap()") => Some(Finding {
            rule_id: "RS-CORRECTNESS-001".to_owned(),
            severity: Severity::Warning,
            category: DiagnosticCategory::Correctness,
            confidence: Confidence::High,
            message: "Unwrap can panic at runtime".to_owned(),
            explanation: "Handle the error or document why the invariant makes unwrap infallible.".to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        "binary_expression" if text.contains(".len()") && text.contains("== 0") => {
            let receiver = text.split(".len()").next()?.trim();
            if receiver.is_empty() {
                return None;
            }
            Some(Finding {
                rule_id: "RS-PERFORMANCE-001".to_owned(),
                severity: Severity::Info,
                category: DiagnosticCategory::Performance,
                confidence: Confidence::High,
                message: "Length comparison can use is_empty".to_owned(),
                explanation: "`len() == 0` and `is_empty()` are equivalent for standard containers and avoid a length lookup expression.".to_owned(),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                fix: Some(FixSpec {
                    replacement: format!("{receiver}.is_empty()"),
                    safe: true,
                    title: "Use is_empty".to_owned(),
                }),
            })
        }
        "unsafe_block" => Some(Finding {
            rule_id: "RS-SECURITY-001".to_owned(),
            severity: Severity::Warning,
            category: DiagnosticCategory::Security,
            confidence: Confidence::Medium,
            message: "Unsafe block requires a local safety argument".to_owned(),
            explanation: "Keep unsafe blocks minimal and document the invariants that make the operation sound.".to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        "macro_invocation" if text.trim_start().starts_with("panic!") => Some(Finding {
            rule_id: "RS-RUNTIME-001".to_owned(),
            severity: Severity::Info,
            category: DiagnosticCategory::Correctness,
            confidence: Confidence::Medium,
            message: "panic! is used in production code".to_owned(),
            explanation: "Return a typed error when callers can recover; reserve panic for unreachable invariants.".to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        _ => None,
    }
}

fn c_rule(source: &str, node: Node<'_>) -> Option<Finding> {
    let text = node_text(source, node);
    if node.kind() == "binary_expression" && text.contains("strlen(") && text.contains("== 0") {
        let start = text.find("strlen(")? + "strlen(".len();
        let end = text[start..].find(')')? + start;
        let argument = text[start..end].trim();
        if argument.is_empty() {
            return None;
        }
        return Some(Finding {
            rule_id: "C-PERFORMANCE-001".to_owned(),
            severity: Severity::Info,
            category: DiagnosticCategory::Performance,
            confidence: Confidence::Medium,
            message: "strlen equality can test the first byte".to_owned(),
            explanation: "This avoids scanning the full string, but requires a documented non-null precondition.".to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: Some(FixSpec {
                replacement: format!("{argument}[0] == '\\0'"),
                safe: false,
                title: "Use first-byte empty check".to_owned(),
            }),
        });
    }
    if node.kind() != "call_expression" {
        return None;
    }
    let (rule_id, message, explanation) = if text.starts_with("gets(") {
        (
            "C-SECURITY-001",
            "gets has no bounds checking",
            "Replace gets with fgets or a bounded parser.",
        )
    } else if text.starts_with("strcpy(") {
        (
            "C-SECURITY-002",
            "strcpy can overflow the destination buffer",
            "Prefer snprintf or an explicitly checked bounded copy.",
        )
    } else if text.starts_with("sprintf(") {
        (
            "C-SECURITY-003",
            "sprintf can overflow the destination buffer",
            "Prefer snprintf with an accurate destination size.",
        )
    } else if text.starts_with("system(") {
        (
            "C-SECURITY-004",
            "system executes a command string",
            "Use an exec-family API with structured arguments when possible.",
        )
    } else {
        return None;
    };
    Some(Finding {
        rule_id: rule_id.to_owned(),
        severity: Severity::Error,
        category: DiagnosticCategory::Security,
        confidence: Confidence::High,
        message: message.to_owned(),
        explanation: explanation.to_owned(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        fix: None,
    })
}

fn javascript_rule(source: &str, node: Node<'_>) -> Option<Finding> {
    let text = node_text(source, node);
    match node.kind() {
        "variable_declaration" if text.trim_start().starts_with("var ") => Some(Finding {
            rule_id: "JS-MODERNIZE-001".to_owned(),
            severity: Severity::Warning,
            category: DiagnosticCategory::Maintainability,
            confidence: Confidence::Medium,
            message: "var has function scope and permits redeclaration".to_owned(),
            explanation: "Use let or const when hoisting behavior is not required. Verify closure capture before applying.".to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.start_byte() + 3,
            fix: Some(FixSpec {
                replacement: "let".to_owned(),
                safe: false,
                title: "Replace var with let".to_owned(),
            }),
        }),
        "binary_expression" => {
            let mut cursor = node.walk();
            let operator = node.children(&mut cursor).find(|child| {
                matches!(child.kind(), "==" | "!=")
            })?;
            let replacement = if operator.kind() == "==" { "===" } else { "!==" };
            Some(Finding {
                rule_id: "JS-CORRECTNESS-001".to_owned(),
                severity: Severity::Warning,
                category: DiagnosticCategory::Correctness,
                confidence: Confidence::Medium,
                message: "Loose equality performs implicit coercion".to_owned(),
                explanation: "Strict equality is usually safer, but verify intentional null/undefined coercion before applying.".to_owned(),
                start_byte: operator.start_byte(),
                end_byte: operator.end_byte(),
                fix: Some(FixSpec {
                    replacement: replacement.to_owned(),
                    safe: false,
                    title: "Use strict equality".to_owned(),
                }),
            })
        }
        "call_expression" if text.starts_with("console.log(") => Some(Finding {
            rule_id: "JS-STYLE-001".to_owned(),
            severity: Severity::Hint,
            category: DiagnosticCategory::Style,
            confidence: Confidence::Medium,
            message: "console.log may be debug output".to_owned(),
            explanation: "Route diagnostics through the application logger in production code.".to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        _ => None,
    }
}

fn java_rule(source: &str, node: Node<'_>) -> Option<Finding> {
    let text = node_text(source, node);
    match node.kind() {
        "catch_clause" if text.contains("{}") => Some(Finding {
            rule_id: "JAVA-CORRECTNESS-001".to_owned(),
            severity: Severity::Warning,
            category: DiagnosticCategory::Correctness,
            confidence: Confidence::Medium,
            message: "Empty catch block swallows failures".to_owned(),
            explanation: "Log, translate, or explicitly document why the exception can be ignored."
                .to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        "method_invocation" if text.starts_with("System.out.print") => Some(Finding {
            rule_id: "JAVA-MAINTAINABILITY-001".to_owned(),
            severity: Severity::Hint,
            category: DiagnosticCategory::Maintainability,
            confidence: Confidence::Medium,
            message: "System.out is usually not suitable for application logging".to_owned(),
            explanation: "Use a logging facade and structured context in production code."
                .to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        "binary_expression" if text.contains("==") && text.contains('"') => {
            let (left, right) = text.split_once("==")?;
            let left = left.trim();
            let right = right.trim();
            let replacement = if left.starts_with('"') && left.ends_with('"') {
                format!("{left}.equals({right})")
            } else if right.starts_with('"') && right.ends_with('"') {
                format!("{right}.equals({left})")
            } else {
                return None;
            };
            Some(Finding {
                rule_id: "JAVA-CORRECTNESS-002".to_owned(),
                severity: Severity::Warning,
                category: DiagnosticCategory::Correctness,
                confidence: Confidence::Medium,
                message: "String identity comparison is suspicious".to_owned(),
                explanation: "Value comparison with a string literal should use equals; the generated edit still requires a type check.".to_owned(),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                fix: Some(FixSpec {
                    replacement,
                    safe: false,
                    title: "Compare by string value".to_owned(),
                }),
            })
        }
        _ => None,
    }
}

fn go_rule(source: &str, node: Node<'_>) -> Option<Finding> {
    let text = node_text(source, node);
    match node.kind() {
        "call_expression" if text.starts_with("strings.Index(") => {
            let parent = node.parent()?;
            let parent_text = node_text(source, parent);
            if parent.kind() == "binary_expression" && (parent_text.contains(">= 0") || parent_text.contains(">=0")) {
                let args = text
                    .strip_prefix("strings.Index(")?
                    .strip_suffix(')')?;
                Some(Finding {
                    rule_id: "GO-PERFORMANCE-001".to_owned(),
                    severity: Severity::Info,
                    category: DiagnosticCategory::Performance,
                    confidence: Confidence::High,
                    message: "Use strings.Contains for substring presence".to_owned(),
                    explanation: "`strings.Index(s, sub) >= 0` computes an index that is not otherwise used.".to_owned(),
                    start_byte: parent.start_byte(),
                    end_byte: parent.end_byte(),
                    fix: Some(FixSpec {
                        replacement: format!("strings.Contains({args})"),
                        safe: true,
                        title: "Use strings.Contains".to_owned(),
                    }),
                })
            } else {
                None
            }
        }
        "call_expression" if text.starts_with("panic(") => Some(Finding {
            rule_id: "GO-RUNTIME-001".to_owned(),
            severity: Severity::Info,
            category: DiagnosticCategory::Correctness,
            confidence: Confidence::Medium,
            message: "panic is used in production code".to_owned(),
            explanation: "Return an error when callers can recover; keep panic for unrecoverable startup failures.".to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        "call_expression" if text.starts_with("fmt.Print") => Some(Finding {
            rule_id: "GO-MAINTAINABILITY-001".to_owned(),
            severity: Severity::Hint,
            category: DiagnosticCategory::Maintainability,
            confidence: Confidence::Low,
            message: "Direct print call may be debug output".to_owned(),
            explanation: "CLI entry points may intentionally print; libraries generally return data or errors.".to_owned(),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            fix: None,
        }),
        _ => None,
    }
}

fn node_text<'a>(source: &'a str, node: Node<'_>) -> &'a str {
    source.get(node.start_byte()..node.end_byte()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_none_comparison_has_safe_fix() {
        let findings = analyze_source(
            "python-builtin",
            Language::Python,
            Path::new("example.py"),
            "if value == None:\n    pass\n",
        );
        let finding = findings
            .iter()
            .find(|item| item.rule_id == "PY-CORRECTNESS-001")
            .expect("finding");
        assert_eq!(finding.fixes.len(), 1);
        assert_eq!(finding.fixes[0].edits[0].replacement, "value is None");
    }

    #[test]
    fn broken_rust_is_reported_as_parse_error() {
        let findings = analyze_source(
            "rust-builtin",
            Language::Rust,
            Path::new("example.rs"),
            "fn main( {",
        );
        assert!(findings.iter().any(|item| item.rule_id == "PARSE-001"));
    }
}
