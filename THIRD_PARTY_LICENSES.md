# Third-party licenses

CodeForge Engine is Apache-2.0 licensed. The Cargo and pnpm lockfiles are the
authoritative dependency inventories.

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
not a default or bundled dependency.
