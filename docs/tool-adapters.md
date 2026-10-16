# Tool Adapters

CodeForge orchestrates local language tools; it does not bundle heavyweight
language servers and does not replace mature formatters.

| Language | Tools |
|---|---|
| Rust | `rustfmt`, `clippy` |
| Python | Ruff format, Ruff check |
| JavaScript / TypeScript | Biome, Prettier, ESLint, Oxlint |
| C / C++ | `clang-format`, `clang-tidy` |
| Go | `gofmt`, `gofumpt`, Staticcheck |
| Java | google-java-format, Spotless, Maven/Gradle verification |
| Dart / Flutter | `dart format`, `dart analyze`, `flutter analyze`, `flutter test` |
| PowerShell | PSScriptAnalyzer through `pwsh` |
| Markdown | markdownlint |
| JSON / YAML / CSS / HTML | Biome or Prettier where appropriate |

Every command is represented as an executable plus an argument array. User
paths are passed as arguments, never interpolated into a shell command string.
External tools are discovered from local paths and workspace-local tool
directories. A missing tool is reported as `unavailable`; it does not prevent
the rest of the fleet from being audited.

Default timeouts are 60 seconds for formatting, 120 seconds for linting, 300
seconds for typecheck/build, 600 seconds for tests, and 900 seconds for
benchmarks. Output is capped and cancellation kills the child process.
