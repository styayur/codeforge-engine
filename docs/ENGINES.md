# Analysis engines

Adapters retain language-native grammars and tooling. A successful parse is
syntax evidence; a review rule can still be heuristic. Coverage is deliberately
selective, not a full compiler or whole-program analyzer for every language.

| Language/domain | Built-in coverage | Optional locally installed tools |
|---|---|---|
| Python | Tree-sitter; selected identity/exception/print rules | Ruff, Pyright, mypy, pytest |
| Rust | Tree-sitter; unwrap/unsafe/panic/length review | Cargo, Clippy, rustfmt, rust-analyzer |
| C | Tree-sitter; selected string/API review | Clang, clang-tidy, clang-format |
| JavaScript / TypeScript | Native grammars; equality/console/style review | Oxlint, Oxfmt, Node/package scripts |
| Java | Tree-sitter; exception/string/output review | javac, Maven, Gradle |
| Go | Tree-sitter; selected string/panic/output review | Go, gofmt, Staticcheck |
| Dart / Flutter | Parsing and selected review | Dart analyze/format, Flutter tests |
| PowerShell | Parsing and selected review | pwsh, PSScriptAnalyzer |
| Markdown / JSON / YAML / TOML / HTML / CSS | Structural and configuration checks; coverage varies | Available local format/lint adapters |

The Rust length-to-`is_empty()` suggestion is **medium-risk heuristic**, not an
automatic safe AST fix. Method names alone do not establish equivalent behavior.
Other suggested transformations also require inspection of their risk and
evidence; this table is not a universal safe-fix matrix.

## Optional tool status

```bash
codeforge doctor .
codeforge review . --language rust --engine clippy --external
```

The second command requires an installed tool and a suitable project. Tool
discovery can run local version probes; missing executables are reported and do
not prevent built-in analysis. Discovery metadata is not proof that every tool
has a complete transformation or formal-verification integration. In particular,
Alive2/OpenRewrite availability must not be read as a bundled semantic verifier.

CodeQL uses a separate explicit CLI command, not default engine registration.
See [External analysis](external-analysis.md), [tool adapters](tool-adapters.md)
and [third-party licenses](../THIRD_PARTY_LICENSES.md).

Stable rule identifiers are listed in [Rule IDs](rule-ids.md). Context/impact
uses bounded syntax extraction and heuristic relations; see
[Source context](source-context.md) for supported shapes and omissions.
