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

## Plugin permissions

Plugins must declare permissions. Bundled and dynamic adapters are denied network permission by default and cannot edit workspace files unless the manifest and host policy allow it.

## Reporting vulnerabilities

Do not open a public issue for a vulnerability. Use GitHub private vulnerability reporting when enabled, or contact the maintainers through the repository security contact.
