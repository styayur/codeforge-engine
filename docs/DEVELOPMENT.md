# Development

Use current stable Rust, Node.js 22 and pnpm 12, matching CI. Cargo declares Rust
1.90, and CI runs a dedicated workspace check with that toolchain. Windows desktop builds
need Visual Studio C++ Build Tools and WebView2. Linux requires WebKitGTK 4.1,
GTK 3, AppIndicator, librsvg and patchelf as listed in the CI workflow.

## Required checks

```bash
python scripts/check_release_metadata.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --lib --bins --tests
cargo build --release -p codeforge-cli

cd apps/desktop
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm build
pnpm tauri build --no-bundle
```

The [CI workflow](../.github/workflows/ci.yml) runs on Windows and Linux. The
[CodeForge Regression Gate](../.github/workflows/codeforge-regression.yml) uses
the pinned v0.2.1 CLI against the PR's changed files and reviewed baseline.
Keep baseline/test changes separate from documentation cleanup; never reclassify
a new finding only to bypass a failing check.

Optional analyzers are not required for the built-in fixture suite. Actual
external-tool verification requires trusted installed tools and project inputs;
fake-process tests do not replace integration testing.

## Contributing

Keep native language semantics in adapters and shared orchestration in the core.
Preserve protocol compatibility, transactions, rollback and evidence provenance.
Cover diagnostics and transformations with meaningful fixtures; test malformed
output, unavailable tools and timeout behavior at external boundaries.
See [Plugins](PLUGINS.md) before adding an adapter.

Use [Documentation checks](DOCUMENTATION_CHECKS.md) for documentation-only work.
Do not update dependency lockfiles or runtime code as a side effect of editing
examples. [Release preparation](RELEASE_PREPARATION.md) records unresolved 1.0
gates. Historical evidence remains available in the [archive](archive/README.md).
