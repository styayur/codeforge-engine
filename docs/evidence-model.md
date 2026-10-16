# Evidence Model

Verification evidence is multi-dimensional:

```text
VerificationEvidence {
  syntax
  compiled
  tested
  benchmarked
  equivalence
}
```

`benchmarked` is not equivalent to `verified`. A benchmark proves only that a
measured command changed under the recorded environment; it does not prove
semantic equivalence. CodeForge reports the dimensions separately and never
turns them into a single fake quality score.

## Evidence bundle

A repository transaction writes:

```text
.codeforge/reports/<id>/
  report.md
  report.json
  diagnostics.sarif
  patch.diff
  before.json
  after.json
  tools.json
```

Fleet reports default to the configured report directory and use one
subdirectory per repository. The Markdown report includes baseline,
diagnostics, changes, verification, performance, risk, transformation classes,
evidence, and rollback information.

## Benchmark evidence

Benchmark results record samples, mean, median, variance, standard deviation,
minimum, maximum, warm-up count, command, timestamp, and sanitized environment
metadata. Before/after comparisons use the configured sample count, at least
five by default. Without real measurements, CodeForge reports `refactored`, not
`optimized`.

Reports do not include secret values, API keys, environment secrets, usernames,
home directories, or machine identifiers.

`tools.json` records the toolchain snapshot used for evidence collection:
detected tools, versions, normalized executable paths, capabilities, and
install hints for missing tools.
