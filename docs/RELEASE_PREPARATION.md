# v1.0.0 release preparation

**Status: draft, not ready to tag solely on documentation approval.** Runtime,
frontend and desktop package versions remain 0.2.2. This documentation change
does not create a compatibility guarantee, tag or release binary.

## Prepared in this documentation stage

- User-facing README, canonical guides and Stable/Experimental/Future labels.
- Evidence-preserving history archive and honest performance methodology.
- [Changelog](../CHANGELOG.md) and [user-facing release draft](releases/v1.0.0.md).
- [Documentation verification procedure](DOCUMENTATION_CHECKS.md).

## Required before publication

- Merge the reviewed documentation PR only after its original CI and Regression Gate are green.
- Align Cargo/workspace, frontend and Tauri package metadata to 1.0.0 in a separately reviewed release change, including generated lockfile version entries as needed.
- Validate CLI/protocol/cache compatibility and state the supported platform/toolchain policy.
- Build final artifacts from the exact reviewed tag candidate; verify installation, uninstall, version output and published checksums.
- Review license notices against final dependency inventories and bundled artifacts; optional external tools are not licensed by this project.

- Validate Rust 1.85 or revise the declared minimum in a separate code/release change.
- Smoke-test Windows/Linux installers, desktop analysis and rollback on the release artifacts; the retained screenshot is a welcome screen, not that evidence.
- Verify that uploaded artifact filenames match `checksums.txt`; GitHub filename normalization previously required checksum-name correction. The current workflow still derives names before upload.
- Review the release workflow's prerelease setting and generated notes before a stable publication; documentation alone does not change that workflow.
- Keep real CodeQL validation and incomplete semantic graph resolution labeled Experimental; fake-CLI tests are not an integration certification.

## Release handling

The GitHub draft uses the proposed `v1.0.0` name but remains unpublished. Do not
push that tag or publish the draft until all release gates above have recorded
evidence. The draft contains no fabricated binary/validation claim. Passing a
documentation PR does not resolve packaging, MSRV or platform-acceptance work.
