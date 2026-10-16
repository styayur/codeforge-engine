# Optional external analysis

Experimental, available in current source; not included in the v0.2.2 package.

`codeforge import-sarif report.sarif --root . --language rust` imports local
SARIF 2.1.0 findings and code-flow paths as JSON. `diagnostic_index` associates
path steps with findings. The importer accepts standard camelCase and older
CodeForge snake_case fields; exports now use the standard wire format.

`codeforge codeql --program C:\tools\codeql\codeql.exe --database D:\db --queries D:\queries --root .`
explicitly invokes an existing executable/database/local query input. It reuses
VerificationPipeline, one thread, a 1024 MiB CodeQL memory hint, a 300-second
configurable timeout, 1 MiB per output stream and a fresh temporary SARIF file.
No default engine registration, download, database creation or service occurs.
The executable may itself access the network or modify its database; this is
not an OS sandbox. Only run a trusted installation with locally resolved packs.
The runner kills the direct child on timeout and bounds inherited output pipes;
it does not guarantee termination of all descendant processes. CLI RAM is a
hint, not an OS-enforced limit.

Import limits: 16 MiB input, 10,000 findings and 100,000 path steps. Unsupported
URI bases are rejected; `%SRCROOT%` explicitly means the supplied root. Source
URIs must remain repository-relative (existing absolute files are canonicalized
against the root). Artifact-index-only and logical-only finding locations are
unsupported. Unsupported path steps produce warnings. Source byte offsets are
unknown (zero); line/column evidence comes from the producer and is not checked
against a source revision. Imported fixes never receive application authority.

Evidence levels remain separate: syntax graph/test suggestions are heuristic;
SARIF is static analysis; build/test gates require actual successful commands;
none of these establishes semantic equivalence. Baseline, transactions, rollback
and Fleet verification remain authoritative. Default scan/review is unchanged.

CodeQL CLI has separate proprietary distribution/use terms, including eligibility
restrictions: [CodeQL CLI license](https://github.com/github/codeql-cli-binaries/blob/main/LICENSE.md).
Users supply their own eligible installation and licensed database/query inputs.
No upstream implementation, binary or query pack is bundled. No Rust dependency
was added. Protocol tests cover standard/legacy wire formats; a local fake CLI
checks argument handling, valid output, nonzero exits and malformed evidence.
A real CodeQL installation was not available for integration validation.
