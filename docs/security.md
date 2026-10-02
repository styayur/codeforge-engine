# Security

## Defaults

- Filesystem-local operation.
- No telemetry.
- No source upload.
- No cloud dependency.
- Network-backed AI is disabled.
- Transformations require a preview and remain reversible.

## Process execution

External tools are executed without a shell:

```text
[executable, arg1, arg2, ...]
```

The runtime applies timeout, controlled environment, bounded output, and explicit executable paths. `shell=true`-style command construction is not supported.

## Fleet mode

Fleet runs are local-only. Source snippets, diagnostics, and reports are not
uploaded by CodeForge. A GitHub PR is created only when `--open-pr` is passed
explicitly and only through the locally installed `gh` CLI. Reports redact
secret values and sanitize benchmark environment metadata; secrets and machine
identifiers are never written to evidence bundles.

## AI boundary

AI is disabled by default. If enabled for proposal generation in a future
release, AI output is untrusted input: it may propose a transformation but
cannot bypass the parser, diff, verification, transaction, or policy boundary.
AI is optional proposal generation, not the trust boundary.

## Plugin permissions

Plugins must declare permissions. Bundled and dynamic adapters are denied network permission by default and cannot edit workspace files unless the manifest and host policy allow it.

## Reporting vulnerabilities

Do not open a public issue for a vulnerability. Use GitHub private vulnerability reporting when enabled, or contact the maintainers through the repository security contact.
