# Benchmarks

Run the existing suites:

```bash
cargo bench -p codeforge-core --bench runtime
cargo bench -p codeforge-fleet --bench fleet
```

Criterion writes to `target/criterion`. See the current
[benchmark report](../docs/benchmarking.md) for methodology, all recorded startup
results and limits. Historical captures are preserved under
[`docs/archive/benchmark-history`](../docs/archive/benchmark-history).
Keep new release-quality captures with their exact command, revision, hardware
and environment; never replace raw history with a best-case summary.
