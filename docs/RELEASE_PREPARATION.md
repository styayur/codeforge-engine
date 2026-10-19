# v1.0.0 release preparation

**Status: release candidate.** Cargo, desktop and Tauri metadata now identify
1.0.0. Publication remains blocked until the reviewed PR, candidate builds and
platform-acceptance evidence below are complete.

## Completed in the release candidate

- Aligned Cargo workspace, desktop package, Tauri bundle and lockfile versions.
- Added a release metadata checker used by CI and release preflight.
- Replaced the unsupported Rust 1.85 declaration with Rust 1.90, the highest
  declared requirement in the locked dependency graph, and added an MSRV job.
- Added CLI `apply`, `history` and `undo` on the same core transaction semantics
  as the desktop application, including preview-only default behavior, ambiguous
  selector refusal, review-required fix approval and post-application hash checks.
- Added CLI round-trip tests for preview, apply, history and undo.
- Added normalized CLI archives, tag-derived stable/prerelease publication,
  curated release notes, deterministic checksums and post-publication byte checks.
- Recorded protocol compatibility boundaries: baseline schema 1 is unchanged,
  SARIF remains 2.1.0, and CLI additions do not remove existing fields or commands.

## Required before publication

- [ ] Required PR CI and Regression Gate are green.
- [ ] Release preflight from the final `main` commit is green.
- [ ] Windows candidate installer installs, starts, analyzes, previews/applies and
      recovers a change, uninstalls, and reports the expected version.
- [ ] Linux candidate CLI runs in a real Linux environment; installer launch is
      recorded as passed or as an environment-specific limitation.
- [ ] Published asset filenames and bytes match `checksums.txt`.
- [ ] Release notes describe the exact tested revision and surviving limitations.

## Experimental boundaries

Context extraction, heuristic impact/test suggestions, local SARIF path import and
optional CodeQL CLI execution remain Experimental. Fake-CLI tests do not certify a
real CodeQL installation or semantic equivalence. Missing required tools remain
execution gaps rather than passing evidence.

## Release handling

The GitHub draft uses `v1.0.0` but remains unpublished until the gates above are
recorded. Do not push the tag or publish the draft merely because documentation or
metadata checks pass.
