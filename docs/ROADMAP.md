# Roadmap and maturity

## Stable workflows in the current source

Native parsing/review, reviewed baselines, transformation previews, transaction
history, configurable verification, SARIF export and independent Fleet tasks
have regression coverage. A stable workflow can carry heuristic findings; its
existence does not make every proposed fix safe.

## Experimental capabilities

On-demand context/impact and external SARIF path/CodeQL commands are available
from source, absent from the v0.2.2 package. Impact resolution is name-based and
bounded; test suggestions cannot replace a full test gate. Real CodeQL execution
has not been validated. Desktop package build success alone does not establish
end-to-end acceptance on every supported platform.

## Before v1.0.0

Resolve the [release checklist](RELEASE_PREPARATION.md): consistent version
metadata, minimum-toolchain validation, green CI, platform installation/desktop
smokes, artifact checksums and a reviewed compatibility/support statement.
Keep experimental features labeled in the release rather than expanding scope.

## Future, not committed release scope

Incremental parse caching, better scope/type-aware relations, a complete worker
SDK and additional equivalence-verifier integrations require separate evidence
and design review. Heavy tools remain optional external processes. No shared
core crate, always-on service, database or LCE dependency is planned here.
