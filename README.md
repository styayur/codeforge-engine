# CodeForge Engine

**Project stage:** Alpha. Implemented workflows have automated verification; independent installation, upgrade and sustained user validation have not been audited.

**Local, CLI-first code analysis and change verification.** CodeForge Engine
finds review candidates, shows the source evidence behind them, previews changes,
runs the checks a project declares, and records what actually happened. It is for
developers who need a small offline engineering tool rather than an IDE or an
autonomous coding agent.

[![CI](https://github.com/styayur/codeforge-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/styayur/codeforge-engine/actions/workflows/ci.yml)
[简体中文](README.zh-CN.md) · [Documentation](docs/README.md) · [Latest published release: v1.1.1](https://github.com/styayur/codeforge-engine/releases/tag/v1.1.1) · [Changelog](CHANGELOG.md)

Examples below use the published v1.1.1 CLI. Subsequent default-branch changes are not automatically included in that release.

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
| Experimental | Context extraction and impact/test suggestions | Exact excerpts; heuristic graph relations, bounded coverage |
| Experimental | SARIF path import and optional CodeQL CLI adapter | Untrusted external evidence; real CodeQL integration has not been validated |
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

Download the CLI archive for your platform from the
[v1.1.1 release](https://github.com/styayur/codeforge-engine/releases/tag/v1.1.1),
verify it against `checksums.txt`, and extract `codeforge` (`codeforge.exe` on
Windows). Desktop packages are available on the same page. Windows requires
WebView2; Linux desktop builds require WebKitGTK. Packages do not install optional
language tools.

To build the CLI from the tagged source instead:

```bash
git clone --branch v1.1.1 --depth 1 https://github.com/styayur/codeforge-engine.git
cd codeforge-engine
cargo install --locked --path crates/codeforge-cli
codeforge --version
```

The workspace declares Rust 1.90 as its minimum supported version and CI checks
that declared MSRV. See [Development](docs/DEVELOPMENT.md) for platform prerequisites.

## CLI examples

Run these from the repository you want to inspect:

```bash
codeforge scan .
codeforge review . --json
codeforge review . --sarif > codeforge.sarif
codeforge refactor .
codeforge apply . --rule RS-PERFORMANCE-001 --file src/lib.rs
codeforge apply . --rule RS-PERFORMANCE-001 --file src/lib.rs --apply --allow-unsafe
codeforge history .
codeforge undo . --transaction <transaction-id>
codeforge verify .
codeforge baseline show --file .codeforge/baseline.json
codeforge ci . --changed --baseline .codeforge/baseline.json --fail-on new
```

`refactor` shows proposals. `apply` is preview-only by default and refuses an
ambiguous selector; `--apply` records a reversible transaction after syntax
verification. Review-required fixes need the explicit `--allow-unsafe` flag.
`undo` refuses to overwrite a workspace whose files changed after application.
`verify` runs trusted project commands, which may write build output or access the
network. Review configuration before executing commands in an unfamiliar
repository. See [Verification](docs/VERIFICATION.md) and
[baseline setup](docs/baseline.md); a baseline must be reviewed, not generated
merely to make CI green.

Experimental commands included in the v1.1.1 package:

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

## Known limitations

Context and impact relations are bounded heuristics, not complete semantic resolution. External SARIF remains untrusted evidence and real CodeQL integration is unvalidated. A successful verification command proves only what that command checks; it is not program-equivalence proof. Desktop installation and sustained use across machines are separate from build checks.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [Development](docs/DEVELOPMENT.md) for existing tests and platform prerequisites. Use [SECURITY.md](SECURITY.md) for vulnerabilities. [Roadmap](docs/ROADMAP.md) separates release gates and future directions; this overview does not promise delivery dates.

## License

CodeForge is independently usable with no LCE runtime or crate dependency. It uses [Apache-2.0](LICENSE); optional tools retain their [own licenses](THIRD_PARTY_LICENSES.md). The current LICENSE restores the canonical Apache text and appendix while retaining the existing copyright notice. This corrects a nonstandard transcription, not a new license choice; historical releases and tags remain unchanged. See [license history](docs/LICENSE_HISTORY.md).
