# 0.2.2

Fleet execution now requires positive evidence for required build/test gates.
Stable task IDs, deterministic PR branches and conservative retry handling
reduce duplicate automation. Task tracing links execution and evidence bundles.
The frontend source-map-js dependency is patched to 1.2.2.

Validated in PR #21: Linux and Windows CI, CodeForge Regression Gate, cargo-audit,
frontend-audit and secret scan. No new Rust dependencies or background services.
This release retains the project's prerelease designation.
