# Toolchain Doctor

```bash
codeforge doctor
codeforge fleet doctor --config fleet.toml
codeforge doctor --json
```

The doctor reports:

- tool id
- required language/capability
- detected or missing
- normalized executable path
- local version when available
- install hint

CodeForge never installs tools, modifies `PATH`, runs package managers, or
requires administrator access during `doctor`. Missing tools such as Dart,
Flutter, or PSScriptAnalyzer are reported as tool gaps instead of source
findings.

Evidence bundles also write `tools.json` with the same toolchain snapshot.
