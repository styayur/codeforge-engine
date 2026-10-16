# Changelog

## v1.0.0

**Unreleased draft.** Package versions remain 0.2.2. Experimental commands below
are implemented in the current source tree but absent from the v0.2.2 package.

### Added

- On-demand source context with exact positions, hashes and bounded output (Experimental).
- Heuristic code relations, change impact and related-test suggestions (Experimental).
- Bounded local SARIF finding/path import and optional user-supplied CodeQL CLI execution (Experimental).
- User guides distinguishing analysis provenance, verification results and application authority.

### Changed

- Current SARIF exports use standard camelCase fields while imports accept legacy CodeForge field names.
- Project documentation centers on analysis, context, transformation, verification and evidence; development history is archived without discarding it.
- Stable workflows, experimental integrations and future scope are documented separately.

### Performance

- Context and external analysis remain opt-in and outside default scan/review/Fleet execution.
- Recorded startup measurements show substantial environment variation; no default scan speedup is claimed.
- The merged CLI is approximately 2.81% larger than the measured original, with no added Rust dependencies. See the benchmark report for scope and raw samples.

### Security

- Imported findings/path evidence cannot authorize fixes or replace build/test gates.
- External process timeout/output limits and SARIF input bounds are explicit; neither is described as an OS sandbox.
- Corrected claims about universal report redaction and the meaning of successful equivalence commands.

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
