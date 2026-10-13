# Lightweight upgrade audit and plan

Baseline: origin/main, Rust 1.98.1, Windows, 2026-10-12. Existing architecture
already has Tree-sitter grammars, Git, workspace ignore handling, Baseline,
SARIF import/export, isolated transactions, rollback, verification subprocesses,
Fleet policy/cache/evidence bundles and tracing. Reuse these modules.

Ordered phases (benefit / complexity / coupling):
1. Require positive build/test evidence when policy requires it; stable task
   identity and deterministic PR branch names, conservative retry behavior,
   execution spans tied to evidence (high / low / Fleet).
2. On-demand source-positioned context and a bounded syntax relation graph with
   impact/test suggestions (medium / medium / engines + CLI). Suggestions are
   heuristic and never replace mandatory tests or prove semantic equivalence.
3. Opt-in external analysis command through existing bounded process runner;
   SARIF/path evidence import (medium / medium / protocol + adapter). Never
   download, install or start CodeQL on ordinary scans.

No cross-repository crate dependency, third core, database, daemon or GUI.
Existing CI and CodeForge Regression Gate remain mandatory. Release only after
all required checks pass; unresolved failures leave the PR unmerged.

License/design references (no source copied or frameworks embedded):
- [Renovate](https://github.com/renovatebot/renovate): AGPL-3.0; deterministic
  task keys and conservative repeat-run behavior.
- [Repowise](https://github.com/repowise-dev/repowise): AGPL-3.0/commercial;
  relationships and evidence-backed impact suggestions.
- [jevgrep](https://github.com/dzhng/jevgrep): MIT; exact source fragments and
  locations. No model service integration.
- [Jeview](https://github.com/andududu/jeview): MIT; execution correlation; use the
  existing tracing dependency, no visualizer integration.
- [CodeQL CLI terms](https://github.com/github/codeql-cli-binaries/blob/main/LICENSE.md):
  CLI is separately licensed; do not redistribute binaries. Users supply their
  own eligible installation/license and prebuilt database/query packs. The MIT
  license on github/codeql source does not license the CLI binary.

## Phase 1 validation

Original workspace tests passed. Fleet tests cover stable keys, sanitization
collisions, malformed/ambiguous PR lookups, required build/test states, optional
failures and three-repository failure isolation. No dependencies or lockfiles
changed. `benchmarks/Measure-Cli.ps1` measures 30 `scan --json` launches after
three warmups, reports mean/P95/peak working set and records a sanitized fixture
summary. This startup workload is a regression guard, not an optimization claim.
