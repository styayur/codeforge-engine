# Changelog

## v1.1.1 - 2026-10-20

### Fixed

- Matched baseline entries created under an older rule-semantics version now enter human review instead of remaining silently accepted.
- Benchmark environment data now resolves Git revision and worktree state from the benchmark project root, and records a clean worktree explicitly.

## v1.1.0 - 2026-10-20

### Added

- Review responses include additive coverage metadata: selected scope and languages, indexed versus selected files, relevant engine availability, external-analysis intent, and explicit coverage limitations.
- The desktop review view shows the coverage strip before diagnostics so a small finding count is not mistaken for repository-wide assurance.
- Benchmark summaries now include P95 latency and record CodeForge/version, Git revision/worktree state, OS, architecture and local Rust tool versions when available.

### Fixed

- Baseline pruning now evaluates the current workspace, defaults to a dry run, removes only resolved entries, retains stale entries for review, requires `--complete-scan --apply`, and refuses writes on ambiguous matches or incomplete scan evidence.
- Rule lifecycle checks now compare an explicit rule semantics version instead of the CodeForge package version.
- Python `None` comparisons now use AST operands, avoiding string literals and chained-comparison rewrites.
- Java empty-catch detection now uses the catch body AST, catches whitespace-empty handlers and skips documented or nonempty handlers.

### Compatibility

- Old review reports without coverage deserialize with an explicit default; existing CLI and desktop consumers keep their previous fields.

## v1.0.0 - 2026-10-19

First stable release of the local, CLI-first analysis and change-verification
workflow. The release keeps optional context, impact and external-analysis paths
Experimental; it does not claim complete semantic resolution or CodeQL integration
certification.

### Added

- CLI `apply` with preview-first, uniquely matched fix selection, explicit approval for review-required fixes, syntax verification and persisted transactions.
- CLI `history` and `undo`, with hash checks that refuse recovery after later workspace edits.
- Standalone CLI archives for Windows x64 and Linux x64, with normalized asset names and SHA-256 checksums.
- Release preflight that verifies tag/version consistency, runs Rust format, Clippy and tests, and validates published asset names and bytes after upload.
- Declared Rust 1.90 MSRV check in CI, reflecting the locked Tree-sitter and Tauri dependency requirements.
- On-demand source context with exact positions, hashes and bounded output (Experimental).
- Heuristic code relations, change impact and related-test suggestions (Experimental).
- Bounded local SARIF finding/path import and optional user-supplied CodeQL CLI execution (Experimental).

### Changed

- Cargo workspace, desktop package, Tauri bundle and lockfile versions now identify the release as 1.0.0.
- Release publication uses the curated release note, a stable/prerelease tag-derived flag and exact normalized filenames.
- CLI transformation entry points share the core preview, verification, transaction and rollback semantics used by the desktop app.
- Current SARIF exports use standard camelCase fields while imports accept legacy CodeForge field names.
- Project documentation centers on analysis, context, transformation, verification and evidence; development history is archived without discarding it.

### Compatibility

- Baseline schema version remains 1; existing exact, structural and context-relocated fingerprints remain supported.
- Existing protocol JSON fields remain readable; new release behavior is additive at the CLI surface.
- SARIF 2.1.0 output remains the compatibility target; imported external evidence cannot authorize fixes.
- Experimental behavior and external-tool requirements remain opt-in.

### Performance

- Context and external analysis remain opt-in and outside default scan/review/Fleet execution.
- Recorded startup measurements show substantial environment variation; no default scan speedup is claimed.
- The earlier measured CLI size increase remains approximately 2.81%; no broader performance claim is made without comparable release measurements.

### Security

- Imported findings/path evidence cannot authorize fixes or replace build/test gates.
- CLI recovery refuses to overwrite files changed after application.
- External process timeout/output limits and SARIF input bounds are explicit; neither is described as an OS sandbox.

## v0.2.2 — 2026-10-13

Published prerelease strengthening Fleet automation and Rust review safety.

### Added

- Stable task identity, deterministic PR branches and task/evidence tracing.

### Changed

- PR retries inspect all PR states and refuse ambiguous lookup or unsafe branch reuse.
- Rust length-to-`is_empty()` proposals are medium-risk heuristic findings, excluded from automatic safe-fix application.

### Performance

- No new Rust dependencies or resident services; startup measurements do not establish a speedup.

### Security

- Fleet requires positive results for policy-required build/test checks and blocks explicit verification failures.
- Frontend source-map-js updated to 1.2.2.

## v0.2.1 - 2026-10-03

Precision and baseline maintenance release.

### Added

- Native `.codeforge/baseline.json` with reviewed dispositions and migration from Fleet baseline JSON.
- Stable exact, structural, and context-relocated finding fingerprints.
- Regression-only CI classification for new, known accepted, known false-positive, human-review, resolved, stale, and ambiguous findings.
- Separate execution status, finding status, source findings, tool gaps, verification failures, and configuration failures.
- `codeforge doctor` and `codeforge fleet doctor` with install hints and read-only toolchain snapshots.
- `tools.json` in evidence bundles.
- Source-context classification and native diagnostic provenance.
- Native machine-readable parsers for Clippy, Ruff, Dart analyze, and PSScriptAnalyzer.
- Precision regression fixtures and baseline lifecycle tests.

### Fixed

- TOML validation now respects real TOML table scope and no longer reports valid repeated keys across tables.
- Rust test and fixture findings are downgraded without blanket suppression.
- Unsafe Rust findings distinguish documented safety boundaries from unjustified unsafe blocks.

## v0.2.0 - 2026-10-02

Verification-driven repository fleet refactoring preview.

### Added

- `codeforge fleet` orchestration with `audit`, `format`, `review`, `refactor`, `optimize`, `verify`, and `report`.
- Stable `fleet.toml` configuration for repository selection, policy, protected paths, and branch naming.
- `codeforge-fleet` crate for discovery, project detection, independent repository transactions, partial-success scheduling, and evidence generation.
- Dart / Flutter and PowerShell adapters, plus Markdown, JSON, YAML, TOML, HTML, and CSS structural review.
- Unified local tool adapter descriptions for rustfmt, clippy, Ruff, Biome, Prettier, ESLint, Oxlint, clang-format, clang-tidy, gofmt, gofumpt, Staticcheck, google-java-format, Spotless, Dart, Flutter, PSScriptAnalyzer, and markdownlint.
- Transformation classes and risk policy: style-only, safe AST fix, dead code, complexity, API refactor, performance candidate, architecture change, and dependency update.
- Evidence bundles with Markdown, JSON, SARIF, patch, before/after snapshots, verification dimensions, and benchmark metadata.
- Read-only `codeforge ci` mode and a GitHub Actions example.
- Fleet dashboard in the Tauri desktop workbench.

### Safety

- Preview-first fleet transformations with explicit `--apply` or `--open-pr`.
- One repository per transaction and per pull request.
- Clean-tree apply policy, protected path checks, generated-code exclusion, diff budgets, and rollback evidence.
- No telemetry, source upload, LLM dependency, dependency upgrade, or fake quality score.

## v0.1.0 - 2026-10-01

Initial usable preview release.

### Added

- Rust workspace shared by CLI and Tauri desktop app.
- Workspace indexing, language detection, Git changed-file filtering.
- Built-in Tree-sitter adapters for Python, Rust, C, JavaScript, TypeScript, Java, and Go.
- Unified diagnostics, fixes, transformations, verification, benchmark, task, engine, and plugin contracts.
- Preview-first transactional edits with persistent Apply/Undo history.
- Sandbox optimization flow with verification and measured comparisons.
- SARIF 2.1.0 export and import model.
- Structured external process execution with timeout and output limits.
- Windows and Linux CI/release workflow configuration.
