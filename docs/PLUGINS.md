# Plugins and engine extensions

CodeForge provides shared contracts, native adapters and declarative engine
metadata. It does not yet offer a complete arbitrary-plugin installation/worker
SDK. Adding a manifest alone does not implement or register a new engine.

## Manifest contract

Use the existing [Rust manifest](../engines/rust/engine.toml) as a concrete example.
Fields include identity, name, semantic version, kind, languages, capabilities,
executable, dependencies, timeout and permissions. Supported kinds are `bundled`,
`local_executable`, `dynamic_adapter` and `external_worker`; their presence in the
schema is not a promise of automatic loading for every kind.

Permission declarations are `read_workspace`, `write_workspace`,
`execute_process` and `network`. Validation checks identifiers, versions,
languages and allowed permission combinations. Bundled manifests are read-only;
bundled/dynamic manifests cannot request network permission. These are metadata
policy checks, **not OS restrictions on an external process**.

## Implement an adapter

1. Reuse a native parser or an explicitly configured local executable.
2. Add metadata/discovery and implementation through `codeforge-engines`.
3. Build structured executable/argument commands through the existing runner.
4. Validate untrusted output and convert it into protocol diagnostics/evidence.
5. Produce previews through existing transformation/transaction boundaries.
6. Test unavailable tools, timeout, malformed output and path handling.
7. Document tool installation, license and what its results do not establish.

Do not promote a tool finding to successful build/test/equivalence evidence.
Keep heavy runtimes external and opt-in. Do not require a service, network or a
shared LCE crate. See [Engines](ENGINES.md), [Security](SECURITY.md) and
[Development](DEVELOPMENT.md).
