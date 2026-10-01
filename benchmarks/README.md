# Benchmarks

The Criterion suite lives in `crates/codeforge-core/benches/runtime.rs`.

```bash
cargo bench -p codeforge-core --bench runtime
```

It measures startup, 1,000-file indexing, single-file analysis, incremental review, AST search, and diagnostic aggregation. Results are written by Criterion under `target/criterion`; release-quality runs should be copied to `benchmarks/results/` with the exact environment and command.
