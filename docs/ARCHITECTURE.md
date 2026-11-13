# Architecture

CodeForge joins analysis and change verification through shared contracts while
preserving language-native semantics. CLI and desktop call the same core; no
background service, database or model is required for a CLI invocation.

## Data flow

```text
Repository → Workspace + Git selection → Language engines
                                         ↓
                                    Diagnostics → Baseline classification
                                         ↓
On-demand source context ───────────→ Transformation preview
                                         ↓
                              Verification + benchmark evidence
                                         ↓
                              Policy / transaction / report
```

Context extraction is explicitly requested, not a mandatory review stage.
Benchmarking, external analyzers and configured build/test commands are separate
operations. The hot/warm/cold scheduling categories organize work; they do not
promise incremental Tree-sitter caching or semantic resolution.

## Responsibilities

| Crate | Responsibility |
|---|---|
| `codeforge-protocol` | Diagnostic, transformation, verification and SARIF data |
| `codeforge-workspace`, `codeforge-git` | Enumeration, hashes, language detection and changed-file selection |
| `codeforge-engines` | Native grammars, selected rules, tool discovery and adapters |
| `codeforge-diagnostics`, `codeforge-baseline` | Aggregation, fingerprints and reviewed finding classification |
| `codeforge-scheduler` | Task priority, cancellation and bounded scheduling |
| `codeforge-transform` | Patch preview, transaction history, apply and undo |
| `codeforge-verification`, `codeforge-benchmark` | Bounded process execution and measured results |
| `codeforge-plugin` | Declarative metadata and manifest validation |
| `codeforge-core` | Single-workspace orchestration and on-demand context |
| `codeforge-fleet` | Repository policy, stable tasks, reports and optional PR publication |

## Trust and failure boundaries

A parser finding, imported path or suggested test has no application authority.
Application policy evaluates explicit check results. Fleet adds stricter
required-check and publication gates; a generic verification `accepted` flag
alone is not equivalent to Fleet approval. See [Verification](VERIFICATION.md).

Missing tools, malformed output and timeouts are reported as unavailable or
failed work. Each Fleet repository retains its own transaction and outcome.
Partial success is possible; there is no fleet-wide atomic rollback. Process
limits are not OS isolation, as described in [Security](SECURITY.md).

## Persistence and evidence

Transactions retain original/transformed content and history under
`.codeforge/history/`. Reports contain diagnostics, patches and verification
results. Fleet adds `run.json`, `summary.md`, `tasks.json` and per-repository
bundles under the configured report directory. Task/run/repository identifiers
in existing `tracing` spans associate execution with evidence, without a new
tracing service or dashboard. See [Evidence Model](EVIDENCE_MODEL.md).

The baseline records reviewed debt; it does not suppress evidence or prove a
change safe. No Rust crate or runtime service is shared with LCE. Optional
language tools and CodeQL remain external processes supplied by the user.
