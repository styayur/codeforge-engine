# Transformation Safety

CodeForge separates review, beautification, refactoring, and performance work.
A single transaction must not mix formatting with semantic or performance
changes.

## Transformation classes

| Class | Default risk | Auto-apply |
|---|---|---|
| `STYLE_ONLY` | low | allowed after verification |
| `SAFE_AST_FIX` | low | allowed after verification |
| `DEAD_CODE` | medium | preview and verification required |
| `COMPLEXITY_REDUCTION` | medium | preview and verification required |
| `API_REFACTOR` | high | proposal only |
| `PERFORMANCE_CANDIDATE` | high | proposal only |
| `ARCHITECTURE_CHANGE` | very high | never automatic |
| `DEPENDENCY_UPDATE` | very high | default prohibited |

Risk filtering is enforced by transformation class, not by a repository health
score. Formatting cannot change dependencies, generated code, lock files,
licenses, or architecture.

## Git safety

Fleet apply requires a clean working tree when policy requires it. `--allow-dirty`
only affects review and report operations; it never permits automatic apply.
Every repository uses its own branch, transaction, evidence bundle, and optional
pull request. Force pushes and history rewrites are not supported.

## Protected and generated files

Protected paths are blocked unless `--allow-protected` is explicit. Generated
code is detected from path conventions and generated headers, then excluded by
default. Dependency upgrades are a separate transformation class and are not
performed by beautification or refactoring.

## Budgets and rollback

The default diff budget is 40 files and 2500 changed lines. Exceeding either
budget returns `SPLIT_REQUIRED` instead of applying changes. Source transactions
write an atomic manifest to `.codeforge/history` before file replacement and
roll back already-written files on failure.
