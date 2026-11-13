# SARIF integration

## Export diagnostics

```bash
codeforge review . --sarif > codeforge.sarif
codeforge ci . --changed --baseline .codeforge/baseline.json --fail-on new --sarif regressions.sarif --new-only
```

The current source serializes standard SARIF 2.1.0 camelCase fields. Earlier
CodeForge exports, including v0.2.2, may contain legacy snake_case fields; do not
assume an older binary has the current wire-format fix. Baselines classify
findings independently of whether they are exported.

## Import external evidence (Experimental, source only)

```bash
codeforge import-sarif report.sarif --root . --language rust
```

This reads a supplied report and emits JSON without running its producer.
It accepts standard and legacy field forms. Code-flow/thread-flow locations
retain their diagnostic association; they are static producer evidence, not an
observed execution trace or semantic repair proof. Imported fixes receive no
authority to apply edits.

The importer bounds input to 16 MiB, 10,000 findings and 100,000 path steps.
Unsupported URI bases are rejected except `%SRCROOT%`, which means the supplied
root. Existing absolute files must canonicalize within that root. Artifact-index
only and logical-only finding locations are unsupported. Unsupported path steps
produce warnings. Byte offsets are unknown; producer line/column positions are
not validated against a source revision.

## Optional CodeQL process

The explicit `codeforge codeql` command uses a user-supplied executable, existing
database and local queries. It does not install CodeQL, create databases or join
the default scan path. A real CodeQL installation was unavailable for integration
testing; fake-CLI tests validate argument handling and failure behavior only.
See [External analysis](external-analysis.md) for commands, limits and licensing.

Treat source paths and report text as sensitive. See [Evidence Model](EVIDENCE_MODEL.md)
and [Security](SECURITY.md) before publishing reports.
