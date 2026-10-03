# Fleet Mode

CodeForge v0.2 adds a local-first repository fleet runtime without changing the
single-repository transaction model. A fleet run discovers configured
repositories, schedules one independent repository task at a time, and writes
one evidence bundle per repository.

## Configuration

```toml
[fleet]
name = "styayur"
workspace_root = "E:/Repositories"
repositories = [
  "developer-security-workspace",
  "stem-visual-explorer",
  "local-context-engine",
  "codeforge-engine",
]
exclude = ["styayur.github.io"]

[policy]
require_clean_tree = true
one_repository_per_transaction = true
one_repository_per_pr = true
rollback_on_failure = true
max_changed_files = 40
max_changed_lines = 2500
require_tests = true
require_build = true
branch_prefix = "codeforge/"

[policy.protected_paths]
patterns = [
  "LICENSE*",
  "THIRD_PARTY*",
  "NOTICE*",
  "Cargo.lock",
  "package-lock.json",
  "pnpm-lock.yaml",
  "yarn.lock",
  "vendor/**",
  "generated/**",
  "dist/**",
  "build/**",
]
```

`fleet.toml` selects repositories and fleet-wide policy. Each repository may
still provide `.codeforge.toml` for its native verification commands. Project
detection only produces suggested commands; it never executes unknown detected
commands automatically.

## Commands

```bash
codeforge fleet audit --config fleet.toml
codeforge fleet review --config fleet.toml --risk low
codeforge fleet format --config fleet.toml --dry-run
codeforge fleet refactor --config fleet.toml --risk medium --dry-run
codeforge fleet optimize --config fleet.toml --require-benchmark
codeforge fleet verify --config fleet.toml
codeforge fleet report --config fleet.toml --report-dir .codeforge/fleet
```

Transformation commands are preview-only unless `--apply` or `--open-pr` is
provided. `--dry-run` never writes source files. Desktop fleet actions are
read-only previews by design.

## Scheduling and status

Repository tasks run concurrently with a default limit of
`min(CPU / 2, 4)`. Benchmarks are never run in parallel with other benchmark
work. A failure in one repository does not abort the fleet run. The final run
status is `success`, `partial_success`, or `failed`; individual repositories can
be `success`, `findings`, `verification_failure`, `missing_tool`,
`configuration_failure`, `transformation_failure`, or `safety_refusal`.

Run summaries are written to `<report-dir>/<run-id>/run.json` and
`summary.md`. Each repository evidence bundle is written below
`<report-dir>/<run-id>/<repository>/`.

Fleet summaries distinguish execution from findings and split source findings
from tool gaps. A completed run with findings reports `execution_status:
complete` and `finding_status: findings`; missing local tools are listed in
`tool_gaps` and do not inflate the source-finding count.

## Cache

Fleet cache keys include the repository, Git commit, configuration hash, tool
version, and rule version. Tool discovery and parser registries are reused for
the duration of a fleet run instead of starting a fresh runtime per repository.

## Exit codes

```text
0 success
1 findings
2 verification failure
3 configuration failure
4 transformation failure
5 external tool unavailable
6 safety policy refusal
```

Exit code `1` means execution completed and source findings were found. It is
not an execution failure.

Reviewed baseline behavior is documented in [baseline.md](baseline.md) and
[regression-ci.md](regression-ci.md).
