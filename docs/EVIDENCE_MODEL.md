# Evidence model

**Finding a problem is not proof that a fix is correct.** Reports separate the
origin of a claim from the checks actually executed on a candidate.

## Evidence categories

| Category | What it establishes | What it cannot establish |
|---|---|---|
| Heuristic | A rule, name match or test-name convention suggests a concern/relation | Type resolution, complete impact coverage or safe replacement |
| Static analysis | A parser/analyzer reports a property within its model | Universal runtime behavior or repair correctness |
| External analysis | A named external producer supplied diagnostics/path evidence | Trustworthiness, current-source correspondence or permission to apply an imported fix |
| Compiler/test verified | Recorded build/test commands passed for the evaluated candidate | Untested behavior or semantic equivalence |

External analysis describes provenance, not automatically a stronger rank than
built-in static analysis. Exact context excerpts are `syntax-extraction`
evidence with source positions and content hashes; candidate call edges and
related-test suggestions remain heuristic even when a name has only one match.

## Protocol labels and check results

The existing `EvidenceLevel` labels summarize execution; they are not new proof
classes or substitutes for the full result:

| Label | Interpretation |
|---|---|
| `Unverified` | No sufficient successful verification evidence |
| `Heuristic` | Limited syntax/rule evidence |
| `Compiled` | A successful build is used by the current level calculation; inspect typecheck separately |
| `Tested` | Configured tests passed; coverage is limited to those tests |
| `Benchmarked` | Performance evidence is present, not correctness proof |
| `Verified` | A configured equivalence command passed; its scope and trust determine the claim |

There is no bundled general-purpose formal verifier. A command returning zero
can produce a successful equivalence result; the label alone cannot tell you
whether it performed a meaningful proof.

Verification retains individual `syntax`, `typecheck`, `build`, `tests`, `fuzz`,
`differential`, `equivalence` and `benchmark` checks with `NotRun`, `Passed`,
`Failed`, `Unavailable` or `Skipped` status. Missing/skipped checks are not
positive evidence. [Fleet](FLEET.md) requires passed syntax and every mandatory
build/test check; explicit failures block application even for optional checks.

## Review coverage

Review responses include a `coverage` object alongside diagnostics. It records:

- whether the run selected all indexed files or only Git-reported changes;
- requested language filters, indexed files, selected files and submitted files;
- per-language discovered and selected counts;
- relevant engine availability and reasons for missing tools;
- whether external analysis was requested;
- explicit limitations, including that submitted-file counts are not line or
  branch coverage and that zero diagnostics do not prove absence of defects.

Older review reports without this field deserialize with an empty default. The
field is evidence about scope and execution, not a quality score.

## Traceability

Repository evidence bundles can contain `report.md`, `report.json`,
`diagnostics.sarif`, `patch.diff`, `before.json`, `after.json` and `tools.json`.
Fleet stores these under `<report-dir>/<run-id>/<repository>/` and associates
them with `tasks.json`. Stable task identity describes the logical operation;
the run ID describes an attempt. Existing `fleet.task` tracing spans carry task,
run and repository IDs, and evidence events carry the bundle ID.

SARIF path steps are associated with a diagnostic index. They are producer
claims, not a replay of observed execution. Imported source line/column positions
are not checked against a source revision. See [SARIF](SARIF.md).

## Performance evidence

Benchmark records include real samples, warm-up count, command and summary
statistics. Compare the same workload and environment; record variance and
regressions. A faster command does not establish behavior preservation.
See [Benchmarking](benchmarking.md) for existing measurements and limitations.

## Sensitive evidence

Patches, source snippets, paths, transaction contents and captured tool output
can include confidential data. Some tool-path and benchmark metadata are
sanitized, but **there is no universal secret-redaction guarantee**. Inspect
bundles before sharing or publishing them. See [Security](SECURITY.md).
