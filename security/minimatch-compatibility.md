# Development glob dependency remediation

GHSA-g3hv-75wg-m249 affects minimatch >=7 and <11.0.1. Pin the affected
TypeScript-ESLint/ESLint consumers to 11.0.4, whose Node engine is
`20 || >=22`; the repository's Node 22/24 runners satisfy it. Existing
minimatch 3.x CommonJS-function consumers are deliberately unchanged.

These dependencies classify checked-in source/config paths during linting and
TypeScript project discovery; they are not imported by product runtime source.
The security concern is malicious glob preprocessing with optimizationLevel 2.
The targeted consumers use the modern named `Minimatch`/`minimatch` APIs, which
remain available in v11. CI runs the actual lint, compiler and build/browser
suite, plus the bounded compatibility fixture below. No forced peer resolution,
blanket audit ignore, or vulnerable version downgrade is used.

`verify-minimatch.mjs` locates the patched matcher through each installed consumer.
The fixture checks wildcard matching and optimization-level-2 preprocessing.
Each child process has a strict time limit. Run the fixture from the package
directory after a frozen-lockfile install; the lockfile pins resolved versions.

Reference: https://github.com/advisories/GHSA-g3hv-75wg-m249
