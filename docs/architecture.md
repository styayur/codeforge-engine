# Architecture

CodeForge separates protocol and orchestration from language semantics. There is no forced cross-language AST. Each adapter retains its native parser, compiler, test runner, and verifier.

## Runtime layers

```text
UI / CLI
  ↓
Core orchestrator
  ↓
Workspace + scheduler + registry + transaction + verification + benchmark
  ↓
Language adapters and process workers
```

## Hot path

Hot path work is bounded and designed for interactive latency:

- Tree-sitter parse
- incremental syntax diagnostics
- structural pattern search
- quick fixes that are already materialized

JVM startup, full builds, formal verification, large benchmarks, and full test suites are forbidden on this path.

## Warm path

Warm analysis handles explicit review or save flows:

- file/module linting
- Clippy, clang-tidy, Staticcheck, Oxlint
- formatting
- project symbol analysis
- ast-grep transformations

## Cold path

Cold work runs only after an explicit command:

- whole-workspace transformation
- sandbox optimization
- build/test verification
- benchmark and profiling
- fuzzing, differential testing, formal verification
- OpenRewrite and Alive2 adapters

## Source control boundary

Git operations are represented as structured arguments. The core can request branch, changed files, staged files, or a revision range, then analyze only changed code.

## Failure isolation

Each engine is independently evaluated. A missing executable, timeout, malformed output, or compile failure becomes a diagnostic or `VerificationStatus`; it does not crash the daemon or block UI startup.

## Persistence

Transaction history is stored under `.codeforge/history/`. Every record contains the patch, verification evidence, benchmark results, original content, and transformed content so Apply → Undo can be verified and reversed.
