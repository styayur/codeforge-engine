# Engines

## Built-in adapters

The bundled adapters use the native Tree-sitter grammar for each language. They provide parse diagnostics, selected heuristic review rules, and previewable fixes.

| Adapter | Parser | Built-in rules |
|---|---|---|
| Python | tree-sitter-python | `None` identity, bare except, print hints |
| Rust | tree-sitter-rust | `.unwrap()`, unsafe blocks, panic macros, `len() == 0` |
| C | tree-sitter-c | bounded string APIs, `system`, `strlen(...) == 0` candidate |
| JavaScript | tree-sitter-javascript | `var`, loose equality, console output |
| TypeScript | tree-sitter-typescript | loose equality, console output |
| Java | tree-sitter-java | empty catch, string identity, System.out hint |
| Go | tree-sitter-go | Index-as-presence candidate, panic, print hints |

Built-in findings are heuristic unless they are followed by compile, test, or equivalence verification.

## External tools

CodeForge detects and reports local tools but deliberately does not bundle them:

- Python: Ruff, Pyright, mypy, ast-grep
- Rust: Cargo, Clippy, rustfmt, rust-analyzer
- C: Clang, clang-tidy, clang-format, Alive2
- JavaScript/TypeScript: Oxlint, Oxfmt, Node, npm, pnpm
- Java: javac, Java runtime, Maven, Gradle, OpenRewrite through Maven/Gradle
- Go: Go toolchain, gofmt, Staticcheck

When an executable is unavailable, the status includes a reason and the rest of the runtime continues.

## Worker model

JVM and long-running language tools execute as external processes. Commands are built from executable plus arguments, with timeout, output limits, and no shell interpolation.
