# Contributing

## Required checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --lib --bins --tests

cd apps/desktop
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm build
pnpm tauri build --no-bundle
```

## Pull requests

- Keep protocol changes backward-compatible unless a breaking release is planned.
- Add fixture coverage for new diagnostics or transformations.
- Do not report performance improvements without measured data.
- Keep CLI and GUI behavior in the shared core; do not duplicate orchestration.
- Treat external tool output as untrusted and validate it before converting to protocol objects.

## Adding an engine

1. Add a manifest under `engines/<language>/`.
2. Add discovery metadata in `codeforge-engines`.
3. Implement a structured command builder.
4. Parse output into unified diagnostics or verification checks.
5. Add failure tests for missing executable, timeout, and malformed output.
6. Document license and installation requirements.
