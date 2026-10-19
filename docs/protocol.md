# Protocol

The protocol is the stable boundary between the core, CLI, desktop UI, and workers.

## Diagnostic

```rust
Diagnostic {
    engine,
    language,
    rule_id,
    severity,
    category,
    confidence,
    file,
    range,
    message,
    explanation,
    source,
    fixes,
    tags,
}
```

## Review coverage

`ReviewReport.coverage` is an additive object. It reports selection scope,
requested languages, indexed/selected/submitted file counts, per-language
counts, relevant engine status, external-analysis intent and explicit notes.
Legacy reports without `coverage` remain readable through a default value.

## Transformation

```rust
Transformation {
    id,
    engine,
    language,
    title,
    description,
    files,
    edits,
    preconditions,
    risk_level,
    reversible,
}
```

## Verification

```rust
VerificationResult {
    syntax,
    typecheck,
    build,
    tests,
    fuzz,
    differential,
    equivalence,
    benchmark,
}
```

Each check has `not_run`, `passed`, `failed`, `unavailable`, or `skipped` status plus optional command, message, and duration.

## Benchmark

A benchmark result only contains measured samples:

```rust
BenchmarkResult {
    metric,
    before,
    after,
    delta_percent,
    environment,
}
```

## SARIF

`diagnostics_to_sarif` maps unified diagnostics to SARIF 2.1.0. The inverse mapping reconstructs locations and core severity/category/confidence fields.
