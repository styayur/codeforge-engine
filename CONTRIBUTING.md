# Contributing

Open an [issue](https://github.com/styayur/codeforge-engine/issues) with a minimal reproduction, platform, CodeForge version and expected behavior. Remove private paths, source and credentials from shared reports. Report vulnerabilities through [SECURITY.md](SECURITY.md).

Before a pull request, read [Development](docs/DEVELOPMENT.md), [Evidence Model](docs/EVIDENCE_MODEL.md) and the relevant engine documentation. Keep changes focused, add regression evidence for behavior changes, and describe unsupported or untested cases. Do not refresh a baseline merely to hide new findings.

Run the applicable existing checks from the repository root:

```sh
python scripts/check_release_metadata.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --lib --bins --tests
```

Desktop changes also require the frontend and native checks in [Development](docs/DEVELOPMENT.md). GitHub CI checks Windows, Ubuntu, the declared Rust 1.90 MSRV, security audits and the CodeForge regression gate; local checks do not replace those results. Do not include generated reports, build output or unrelated changes.

Contributions retain the repository's [Apache-2.0 terms](LICENSE) and applicable [third-party notices](THIRD_PARTY_LICENSES.md). Existing release artifacts and tags are not rewritten by a documentation change.
