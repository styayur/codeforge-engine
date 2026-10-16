# Upgrade measurements

Windows x64, Rust 1.98.1, release CLI, fixtures/python, three warmups and 30
process samples per build. These runs include tool discovery; they are a startup
regression guard, not evidence of faster static analysis. Local load and PATH
affect elapsed time. Peak working set is the Windows RSS analogue (not private
heap). Raw samples and sanitized fixture summaries are in results/upgrade-20261012.

| Measurement | Original 0.2.1 | Fleet phase | Context + external phase |
|---|---:|---:|---:|
| Mean ms | 3239.811 | 3206.954 | 2699.553 |
| P95 ms | 4373.1 | 5210.27 | 3208.55 |
| Peak working set bytes | 21065728 | 20946944 | 21045248 |
| CLI bytes | 15139328 | 15172096 | 15563264 |

A fresh original-binary run beside phase 3 measured mean 2745.893 ms,
P95 3530.672 ms and peak working set 7692288 bytes.
The sanitized fixture result is identical. No startup speedup is attributed to
these architecture changes; the optional paths are not exercised by scan.
The final CLI grows 423,936 bytes
(2.80%) for the optional commands.

Cargo.lock retains 516 packages, including 501 registry/Git packages. No new
Rust dependencies; frontend source-map-js was patched 1.2.1 -> 1.2.2 with no
new package. Full workspace tests/clippy pass locally. Context tests cover exact
source positions, deterministic bounds, stale content and ambiguity; external
tests cover standard/legacy SARIF, rejected fixes/URI bases, malformed output,
nonzero process exits and inherited-pipe deadlines. These are not semantic
equivalence proofs or a real CodeQL integration benchmark.

The standalone working-set runs varied considerably (about 7.7 vs 21 MB), so a
six-pair interleaved recheck was run with no concurrent local compilation. Raw
readings are in interleaved-memory.json. Do not infer a memory improvement from
the isolated startup runs. Sampled private bytes are not peak RSS.

| Interleaved measurement | Original | Upgraded |
|---|---:|---:|
| Mean elapsed ms | 2791.23 | 2727.22 |
| Mean per-run peak working set bytes | 7214421.33 | 7268352.00 |
| Mean sampled private bytes | 2198869.33 | 2316288.00 |

No substantial default-path runtime or memory regression reproduced. Binary
growth is 2.80%; optional context/SARIF commands account for the added code.
