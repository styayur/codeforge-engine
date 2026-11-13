# Documentation migration inventory

This inventory records documentation organization, not runtime changes.

## Canonical structure

- [ARCHITECTURE.md](ARCHITECTURE.md)
- [ENGINES.md](ENGINES.md)
- [VERIFICATION.md](VERIFICATION.md)
- [EVIDENCE_MODEL.md](EVIDENCE_MODEL.md)
- [FLEET.md](FLEET.md)
- [SARIF.md](SARIF.md)
- [SECURITY.md](SECURITY.md)
- [PLUGINS.md](PLUGINS.md)
- [DEVELOPMENT.md](DEVELOPMENT.md)
- [ROADMAP.md](ROADMAP.md)

## Moved files

| Previous path | Current path |
|---|---|
| `docs/UPGRADE-2026-10.md` | `docs/archive/upgrade-2026-10.md` |
| `docs/architecture.md` | `docs/ARCHITECTURE.md` |
| `docs/engines.md` | `docs/ENGINES.md` |
| `docs/verification.md` | `docs/VERIFICATION.md` |
| `docs/evidence-model.md` | `docs/EVIDENCE_MODEL.md` |
| `docs/fleet-mode.md` | `docs/FLEET.md` |
| `docs/security.md` | `docs/SECURITY.md` |
| `docs/plugin-system.md` | `docs/PLUGINS.md` |
| `docs/contributing.md` | `docs/DEVELOPMENT.md` |
| `docs/release-0.2.2.md` | `docs/archive/development-notes/release-0.2.2.md` |
| `docs/rust-length-rule.md` | `docs/archive/development-notes/rust-length-rule.md` |
| `benchmarks/UPGRADE.md` | `docs/archive/benchmark-history/upgrade-measurements.md` |
| `benchmarks/results/2026-10-01-windows-gnu.json` | `docs/archive/benchmark-history/2026-10-01-windows-gnu.json` |
| `benchmarks/results/upgrade-20261012/before.json` | `docs/archive/benchmark-history/upgrade-20261012/before.json` |
| `benchmarks/results/upgrade-20261012/interleaved-memory.json` | `docs/archive/benchmark-history/upgrade-20261012/interleaved-memory.json` |
| `benchmarks/results/upgrade-20261012/paired-before.json` | `docs/archive/benchmark-history/upgrade-20261012/paired-before.json` |
| `benchmarks/results/upgrade-20261012/phase1.json` | `docs/archive/benchmark-history/upgrade-20261012/phase1.json` |
| `benchmarks/results/upgrade-20261012/phase3.json` | `docs/archive/benchmark-history/upgrade-20261012/phase3.json` |

## Added files and preserved snapshots

- `.markdownlint.json`
- `SECURITY.md`
- `docs/DOCUMENTATION_CHECKS.md`
- `docs/README.md`
- `docs/RELEASE_PREPARATION.md`
- `docs/ROADMAP.md`
- `docs/SARIF.md`
- `docs/archive/README.md`
- `docs/archive/development-notes/architecture-before-v1.md`
- `docs/archive/development-notes/changelog-before-v1.md`
- `docs/archive/development-notes/readme-before-v1.md`
- `docs/archive/development-notes/security-before-v1.md`
- `docs/archive/development-notes/upgrade-delivery-2026-10.md`
- `docs/assets/README.md`
- `docs/releases/v1.0.0.md`
- `docs/DOCUMENTATION_MIGRATION.md` (this inventory).

## Rewritten entry points

README, CHANGELOG and canonical guides now describe user workflows and current
limits. Earlier README/architecture/security/changelog content remains in the
archive. Existing supporting references remain available. Relative links were
updated to follow moved files.

## Removed from current narrative

- PR-by-PR delivery logs and phase plans moved out of the user entry points.
- Unsupported performance comparisons and absolute safety/privacy claims were corrected.
- Obsolete safe-fix/version statements were replaced with current limitations.

No historical report, raw measurement, screenshot, core source, fixture or test
was intentionally deleted. File paths retired by moves are listed above.
Raw moved CSV/JSON/text captures were compared with their original Git contents
and retain identical content (allowing repository newline normalization).

## Verification scope

Markdown lint covers current guides and templates; archives retain historical
formatting. Link checks include archives and image targets. README GFM renders
were checked at desktop and narrow widths. Existing Rust tests/format/Clippy
were run without modifying tests or baseline. Original PR CI and Regression Gate
remain required. See [release preparation](RELEASE_PREPARATION.md) for blockers.
