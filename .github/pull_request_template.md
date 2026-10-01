## What changed

Describe the problem and the implementation.

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --lib --bins --tests`
- [ ] `pnpm lint`
- [ ] `pnpm typecheck`
- [ ] `pnpm build`
- [ ] `pnpm tauri build --no-bundle`

## Risk notes

- [ ] Transformations have preview and undo coverage.
- [ ] No unverified performance claims were added.
- [ ] No telemetry, source upload, or shell command interpolation was introduced.
