# Verification

The verification pipeline is configurable and reports evidence separately from diagnostics.

```text
Parse
  ↓
Syntax validation
  ↓
Typecheck
  ↓
Compile
  ↓
Tests
  ↓
Differential testing
  ↓
Fuzzing
  ↓
Formal verification
  ↓
Benchmark
  ↓
Accept / Reject
```

## Evidence levels

- `Unverified`: no stage passed.
- `Heuristic`: syntax parsing passed but no stronger check did.
- `Compiled`: build/typecheck passed.
- `Tested`: tests passed.
- `Verified`: an equivalence or formal verifier passed.

Tests are not called proofs. A timeout, missing executable, or unsupported verifier is represented explicitly as `failed` or `unavailable`.

## Transformation policy

A transformation preview is rejected before apply when syntax verification fails. Full build/test verification can be run before or after apply. The optimization workflow never writes to the source workspace; it verifies and benchmarks a temporary copy.
