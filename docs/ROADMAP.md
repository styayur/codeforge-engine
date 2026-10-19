# Roadmap and maturity

## Stable workflows in v1.0.0

Native parsing/review, reviewed baselines, CLI and desktop preview/apply/undo,
transaction history, configurable verification, SARIF export and independent
Fleet tasks have regression coverage. A stable workflow can carry heuristic findings; its
existence does not make every proposed fix safe.

## Experimental capabilities

On-demand context/impact and external SARIF path/CodeQL commands are included in
v1.0.0 as Experimental, opt-in paths. Impact resolution is name-based and bounded;
test suggestions cannot replace a full test gate. Real CodeQL execution has not
been certified. Desktop package build success alone does not establish end-to-end
acceptance on every platform.

## Next release candidates

Evidence coverage metadata, baseline lifecycle improvements, local symbol/scope
analysis, project verification orchestration, reproducible before/after benchmark
comparison and executable architecture constraints require separate design and
test evidence. They are not silently promoted by the v1.0.0 release.

## Future, not committed release scope

Incremental parse caching, better scope/type-aware relations, a complete worker
SDK and additional equivalence-verifier integrations require separate evidence
and design review. Heavy tools remain optional external processes. No shared
core crate, always-on service, database or LCE dependency is planned here.
