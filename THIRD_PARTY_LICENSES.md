# Third-party licenses

CodeForge Engine is [Apache-2.0](LICENSE) licensed. The Cargo and pnpm lockfiles
are the dependency inventories. This summary is not a complete distribution
notice bundle; review the exact dependencies and artifacts before publication.

The project currently links or depends on major open-source components including:

- Rust standard ecosystem crates: Apache-2.0 or MIT
- Tauri and Wry: Apache-2.0 or MIT
- Tree-sitter and language grammars: MIT
- Tokio: MIT
- Serde: Apache-2.0 or MIT
- React, Vite, Zustand, TanStack Virtual, Monaco Editor, and lucide-react:
  distributed under their respective upstream licenses.

Optional engines are not redistributed:

- Ruff, Pyright, mypy, LibCST
- Cargo, Clippy, rustfmt, rust-analyzer
- Clang, clang-tidy, clang-format, LLVM, Alive2
- Oxc, Oxlint, Oxfmt, ast-grep
- OpenRewrite, Error Prone, javac, JUnit, JMH
- gopls, gofmt, go vet, Staticcheck, pprof

Review each tool's license before redistributing it in an installer. CodeQL is
not a default or bundled dependency. Its CLI has separate
[GitHub terms](https://github.com/github/codeql-cli-binaries/blob/main/LICENSE.md)
covering eligible use and restricting redistribution. Users supply their own
eligible installation; this repository does not grant rights to distribute it.

Design references do not become linked dependencies. No Renovate, Repowise,
zvec-grep, jevgrep or Jeview implementation/framework was copied or bundled by
the lightweight upgrade. Their licenses do not replace the licenses of the
actual Cargo/frontend dependency inventory.
