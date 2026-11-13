# Security and privacy

## Local defaults

Built-in analysis needs no account, hosted model, telemetry or source upload.
CodeForge does not install optional analyzers. Context and impact commands read
source and Git state without applying changes or starting analyzer/model tools.
Other workflows may execute local version probes during tool discovery.

Offline-first is not a network firewall: configured compilers, package managers,
tests and analyzers may download dependencies or contact services. Fleet
`--open-pr` explicitly uses Git and the locally installed `gh` client to publish.
Use trusted local tools and already-resolved dependencies for offline operation.

## External process boundary

The runner builds executable/argument arrays without implicit shell interpolation,
limits captured output and enforces deadlines. It inherits the parent environment
with configured overrides; credentials present there can be visible to a child.
An explicitly configured shell remains a shell supplied by the user.

Timeout handling kills the direct child and bounds output collection, but does
not guarantee termination of all descendants. There is no OS-enforced filesystem,
network, memory or credential sandbox. Temporary optimization copies isolate
managed source edits; they do not contain arbitrary tool side effects.

## Source changes and Fleet

Preview proposals before applying. Fleet enforces configured clean-tree checks,
protected paths, diff budgets and required verification, with one repository per
transaction. Existing branches are not forcibly reset. Failed or ambiguous PR
lookup refuses publication. Rollback restores managed source changes; external
side effects and multi-repository operations are not atomic.

See [Fleet](FLEET.md), [Verification](VERIFICATION.md) and
[Transformation Safety](transformation-safety.md). Do not assume every interactive
single-workspace operation inherits Fleet's stricter policy.

## Reports, caches and baselines

`.codeforge/history`, reports, SARIF, patches and context excerpts may contain
source text, filesystem paths and tool output. Select metadata is sanitized;
there is no complete secret scanner or universal redaction. Review and restrict
access to these files before uploading them. A reviewed baseline is an audit
record of findings, not a security exemption or proof of safety.

## Plugins and external evidence

Manifest permission checks validate declarations; they are not an OS sandbox.
Only run trusted executables under an appropriately restricted account or your
own isolation environment. [Plugins](PLUGINS.md) explains the implemented scope.

SARIF is untrusted producer input. The importer bounds input size/counts and
rejects unsupported or escaping paths; imported fixes never authorize application.
Source revision correspondence is not established by import. CodeQL is an
optional user-supplied CLI with separate licensing and no default startup.

## Reporting vulnerabilities

See the repository [security policy](../SECURITY.md). Do not publish sensitive
source, credentials or an exploit in a public issue. AI proposal generation is
a future extension, not a current trust or verification boundary.
