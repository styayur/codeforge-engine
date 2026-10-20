# v1.1.0 release preparation

**Status: release candidate, not yet published.** Version metadata is frozen at
1.1.0 and the release workflow can build candidate artifacts without publishing.

## Completed in the candidate

- Baseline pruning removes only resolved entries, retains stale entries, requires
  `--complete-scan --apply`, and blocks ambiguous or incomplete scans.
- Rule lifecycle uses an explicit rule-semantics version rather than the package
  version.
- Python `None` and Java empty-catch rules use AST evidence with positive and
  negative regression tests.
- Review coverage fields expose indexed/selected/submitted counts and engine
  availability in JSON and the desktop review view.
- Benchmark summaries include P95, warm-up/sample counts and environment records
  including Git revision and local Rust tool versions.
- Release workflow explicitly sets the release title from the tag.

## Required before publication

- [ ] Required PR CI, Security and CodeForge Regression gates are green.
- [ ] Non-publishing candidate build succeeds for Windows and Linux.
- [ ] Windows candidate CLI, MSI and NSIS are smoke-tested.
- [ ] Linux candidate CLI, deb and AppImage are smoke-tested in an available
      environment; limitations are recorded.
- [ ] Published assets are redownloaded and SHA-256 verified after release.

## Compatibility

Baseline schema remains 1. SARIF remains 2.1.0. Review coverage is additive.
Destructive baseline pruning is intentionally stricter and requires migration to
`--complete-scan --apply`.

## Historical evidence

The completed v1.0.0 acceptance record is retained at
[v1.0.0 evidence](releases/v1.0.0-evidence.md).
