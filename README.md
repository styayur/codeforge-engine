# CodeForge Engine

**Verified code analysis and transformation engine.** Inspect a repository,
review proposed changes, run its verification commands, and keep the evidence
that explains each decision.

[![CI](https://github.com/styayur/codeforge-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/styayur/codeforge-engine/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

[Documentation](docs/README.md) · [Latest published prerelease: v0.2.2](https://github.com/styayur/codeforge-engine/releases/tag/v0.2.2) · [Changelog](CHANGELOG.md)

**v1.0.0 is an unreleased candidate.** There is no published stable 1.0 binary.
This README describes the current source tree. Context, impact and external
SARIF/CodeQL commands are available from source and are absent from v0.2.2.

## Why CodeForge

A finding is useful only when you can locate its source, understand its limits,
and decide what evidence a proposed fix needs. CodeForge connects **analysis,
context, transformation, verification and evidence** in one local workflow.
It is an engineering tool for reviewing and validating changes, rather than an
AI coding assistant. Lint results are inputs to that workflow, not its endpoint.

Languages retain their native parsers and tools. Built-in analysis works offline;
optional compilers and analyzers come from your own installation. A reviewed
baseline keeps known findings visible while CI identifies new regressions.

## Core capabilities and maturity

Stable below means implemented with regression coverage; it does not imply that
every language rule or suggested transformation is semantically proven.

| Status | Capability | Boundary |
|---|---|---|
| Stable | Multi-language parsing and review | Tree-sitter and selected rules; coverage differs by language |
| Stable | Transformation previews, transactions and rollback | Inspect patches; verification requirements depend on the workflow |
| Stable | Configurable verification pipeline | Records actual command results, including unavailable/skipped checks |
| Stable | Baselines and Fleet automation | Independent repository transactions, deterministic task identity and guarded PR creation |
| Stable | SARIF export and evidence bundles | Current source emits standard SARIF; findings do not authorize edits |
| Experimental, source only | Context extraction and impact/test suggestions | Exact excerpts; heuristic graph relations, bounded coverage |
| Experimental, source only | SARIF path import and optional CodeQL CLI adapter | Untrusted external evidence; real CodeQL integration has not been validated |
| Future | Incremental parse caching, broader semantic resolution and verifier integrations | No installation or execution implied |

See [Engines](docs/ENGINES.md) for language coverage and optional tool status.

## Architecture

```text
Repository
  ↓
Analysis engines → Diagnostics + reviewed baseline
  ↓
Context (on demand; not a mandatory scan stage)
  ↓
Transformation preview → transaction
  ↓
Verification (configured syntax / build / tests / other commands)
  ↓
Evidence + policy decision + apply / rollback
```

The CLI and desktop share the same core. Fleet wraps independent repository
operations; it does not create a cross-repository atomic transaction.
[Architecture](docs/ARCHITECTURE.md) explains boundaries and persistence.

## Evidence model

**Finding a problem is not proof that a fix is correct.** Heuristic suggestions,
static analysis, external analysis and successful compiler/test runs are distinct
evidence. A benchmark measures performance; a configured equivalence command
records what that command checked, not a built-in proof of program equivalence.

Read [Evidence Model](docs/EVIDENCE_MODEL.md) before interpreting a report or
automating application. Fleet requires successful checks mandated by policy;
missing required tools do not count as a pass.

## Installation

Use the desktop packages and `checksums.txt` on the
[v0.2.2 prerelease](https://github.com/styayur/codeforge-engine/releases/tag/v0.2.2)
page for Windows/Linux. Windows requires WebView2; Linux desktop builds require
WebKitGTK. A desktop package does not install optional language tools.

For the current CLI, including the experimental source-only commands:

```bash
git clone https://github.com/styayur/codeforge-engine.git
cd codeforge-engine
cargo install --locked --path crates/codeforge-cli
codeforge --help
```

This builds the checked-out revision, whose package version is currently 0.2.2;
it is not equivalent to checking out tag v0.2.2. For reproducibility, record
`git rev-parse HEAD`. Use current stable Rust; see
[Development](docs/DEVELOPMENT.md) for platform prerequisites.

## CLI examples

Run these from the repository you want to inspect:

```bash
codeforge scan .
codeforge review . --json
codeforge review . --sarif > codeforge.sarif
codeforge refactor .
codeforge verify .
codeforge baseline show --file .codeforge/baseline.json
codeforge ci . --changed --baseline .codeforge/baseline.json --fail-on new
```

`refactor` shows proposals. `verify` runs trusted project commands, which may
write build output or access the network. Review configuration before executing
commands in an unfamiliar repository. See [Verification](docs/VERIFICATION.md)
and [baseline setup](docs/baseline.md); a baseline must be reviewed, not generated
merely to make CI green.

Optional commands from the current source tree:

```bash
codeforge context . --query verification --limit 10 --max-bytes 32768
codeforge impact . --depth 4 --graph
codeforge import-sarif report.sarif --root . --language rust
codeforge fleet audit --config fleet.toml
codeforge fleet refactor --config fleet.toml --risk low --dry-run
```

Supply your own SARIF file or [Fleet configuration](docs/FLEET.md). Context and
impact read source without applying changes; suggested tests never replace the
full verification policy. [SARIF](docs/SARIF.md) covers external evidence limits.

## Security model

No account, telemetry, hosted service or model is required. Local tool discovery
may execute version probes. External commands inherit host permissions and
environment; timeout/output bounds are not an OS sandbox. Reports can contain
paths, source and tool output, so review them before sharing. GitHub publication
is explicit through Fleet PR mode. See [Security](docs/SECURITY.md) and
[vulnerability reporting](SECURITY.md).

## Engine extensions and development

Reuse native parsers and the existing protocol, process runner and transaction
boundaries. [Plugins](docs/PLUGINS.md) distinguishes manifest validation from
actual process isolation; arbitrary manifest installation is not a complete SDK.
[Development](docs/DEVELOPMENT.md) lists CI checks, and
[Roadmap](docs/ROADMAP.md) separates release gates from future work.

[Benchmark methodology and observations](docs/benchmarking.md) include resource
variation and do not claim a default scan speedup.

CodeForge is independently usable with no LCE runtime or crate dependency.
Licensed under [Apache-2.0](LICENSE); optional tools retain their
[own licenses](THIRD_PARTY_LICENSES.md).
