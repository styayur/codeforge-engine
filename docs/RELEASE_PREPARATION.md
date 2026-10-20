# v1.1.1 release evidence

**Status: released.** [CodeForge Engine v1.1.1](https://github.com/styayur/codeforge-engine/releases/tag/v1.1.1)
was published from tag commit `4f065923c9273d0c9907222b07c15a252cd9055c`.

## Governance

- Fix PR: [#32](https://github.com/styayur/codeforge-engine/pull/32).
- Non-publishing candidate build: [39375384698](https://github.com/styayur/codeforge-engine/actions/runs/39375384698).
- Publishing workflow: [39376869109](https://github.com/styayur/codeforge-engine/actions/runs/39376869109).
- CI, Security, Rust 1.90 MSRV and CodeForge Regression gates passed.
- The publishing workflow downloaded all assets again and verified SHA-256 values.

## Fixed from v1.1.0

- Matched baseline entries from older rule semantics now become stale and require human review.
- Benchmark Git provenance is resolved from the benchmark project root, not the process cwd.
- Clean benchmark worktrees are recorded explicitly.

## Acceptance

- Windows CLI `1.1.1`: benchmark from another cwd recorded the target commit and `clean` worktree.
- Windows MSI and NSIS: install, desktop startup and uninstall passed.
- Linux CLI: benchmark from another cwd recorded the target commit and `clean` worktree.
- AppImage: extraction and WSLg startup passed.
- Linux deb: install, WSLg startup and uninstall passed.
- Host limits remain the same as v1.1.0: WSL2/WSLg, not bare metal.

## Published SHA-256

```text
3cef32557a7c6554de8dfaff643fe94bffb68de01792b7555b5010ad413f8b39  CodeForge.Engine_1.1.1_amd64.AppImage
ab1c1114111270f1ff1c52c2dd392ec81cf2c7dc66e98ab4b6a2c266fb27b622  CodeForge.Engine_1.1.1_amd64.deb
1e280bc5f4cd2db8f8cf192d00ed66473dc6db7becc930ec39a22ec02fb0f5fe  CodeForge.Engine_1.1.1_x64-setup.exe
9be9f1a6962f671830e38410bb61bcf62d49fed96ec590b737d836640821c261  CodeForge.Engine_1.1.1_x64_en-US.msi
f3ecd5ec5103b934cd15fadfe9de942ea990ccaecd609d7582d01e2f7c32217b  codeforge-cli-v1.1.1-x86_64-pc-windows-msvc.zip
1403eca00e5d9b0cd2cbe503cb1763993fa226af97dfc3da2620db6ac99dc2f8  codeforge-cli-v1.1.1-x86_64-unknown-linux-gnu.tar.gz
```

## Historical evidence

- [v1.1.0 evidence](releases/v1.1.0-evidence.md)
- [v1.0.0 evidence](releases/v1.0.0-evidence.md)
