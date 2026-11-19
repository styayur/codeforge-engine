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

`verify-minimatch.mjs` resolves minimatch from each affected installed consumer
and verifies named APIs, brace/globstar/exclusion behavior, and bounded
optimization-level-2 preprocessing. Run it from the package directory after a
frozen-lockfile install. The checked lockfile is the installed-version authority.

Reference: https://github.com/advisories/GHSA-g3hv-75wg-m249
