# Plugin system

The plugin system starts with declarative manifests. It does not load arbitrary native code into the daemon process.

## Manifest fields

- `id`: stable alphanumeric/hyphen/underscore identifier
- `name`
- `version`: semantic version
- `kind`: `bundled`, `local_executable`, `dynamic_adapter`, or `external_worker`
- `languages`
- `capabilities`
- `executable`
- `dependencies`
- `timeout_ms`
- `permissions`

## Permissions

- `read_workspace`
- `write_workspace`
- `execute_process`
- `network`

Bundled adapters are read-only by policy. Dynamic adapters may not request network access by default. External workers receive explicit executable paths and argument arrays.

## Discovery

The desktop UI shows all engine statuses, including unavailable optional tools. A plugin failure is isolated and reported through the task engine.
