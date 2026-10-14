# Benchmarking

Benchmarks record real command/engine timing. Before/after comparisons need the
same workload and environment; a timing improvement is not semantic equivalence.

## Existing startup observations

Windows x64, Rust 1.98.1, release CLI, `scan fixtures/python --json`, three
warmups and 30 process samples per build. Tool discovery and local PATH affect
startup. Raw captures do not provide a complete hardware inventory; record CPU,
RAM, disk and exact target for future release-quality measurements.

| Measurement | Original | Fleet stage | Context/external stage |
|---|---:|---:|---:|
| Mean ms | 3239.811 | 3206.954 | 2699.553 |
| P95 ms | 4373.100 | 5210.270 | 3208.550 |
| Peak working set bytes | 21,065,728 | 20,946,944 | 21,045,248 |
| CLI bytes | 15,139,328 | 15,172,096 | 15,563,264 |

A fresh original-binary run measured mean 2745.893 ms, P95 3530.672 ms and peak
working set 7,692,288 bytes. Sanitized fixture output matched. Because isolated
working-set readings varied strongly, a six-pair interleaved check was performed
without concurrent compilation:

| Interleaved measurement | Original | Upgraded |
|---|---:|---:|
| Mean elapsed ms | 2791.23 | 2727.22 |
| Mean per-run peak working set bytes | 7,214,421 | 7,268,352 |
| Mean sampled private bytes | 2,198,869 | 2,316,288 |

No substantial default-path regression reproduced, but these small samples do
not establish a speedup or a memory improvement. Working set is the Windows
resident-memory analogue; sampled private bytes are not peak RSS. Optional
context/SARIF commands are not exercised by this scan benchmark. The merged CLI
was 15,565,312 bytes, approximately 2.81% larger than the original; the stage
binary above is not the final merged binary. Cargo.lock retains 516 packages,
501 external. No Rust dependency was added; the existing frontend source-map-js
received a patch update. This documentation revision has no runtime changes.

Raw [before](archive/benchmark-history/upgrade-20261012/before.json),
[Fleet](archive/benchmark-history/upgrade-20261012/phase1.json),
[context/external](archive/benchmark-history/upgrade-20261012/phase3.json),
[paired original](archive/benchmark-history/upgrade-20261012/paired-before.json)
and [interleaved samples](archive/benchmark-history/upgrade-20261012/interleaved-memory.json)
remain available. The [initial benchmark capture](archive/benchmark-history/2026-10-01-windows-gnu.json)
uses a different harness and is not a like-for-like baseline.

## Reproduce measurements

```bash
cargo build --release --locked -p codeforge-cli
codeforge scan fixtures/python --json
cargo bench -p codeforge-core --bench runtime
cargo bench -p codeforge-fleet --bench fleet
```

The scan command is the workload, not a complete sampling/memory harness.
Repeat warmups/samples externally and record revision, executable size, platform,
tool inventory, sample values and process-memory method. Criterion writes its
own reports under `target/criterion`; do not compare Criterion estimates with
process-startup P95 as if they were the same metric.

## Project benchmark commands

Configure `.codeforge.toml` or use supported detected native benchmark commands.
The runner uses a warm-up and measured samples, recording mean, median, variance,
minimum and maximum. Run `codeforge benchmark . --samples 5` only for a trusted
project. A command may modify files or access the network.

Preserve full measurements and regressions. Record command, hardware, software,
sample count, correctness checks and limitations with every published comparison.
