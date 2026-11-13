# Regression-Only CI

CI is read-only with respect to the reviewed baseline.

```bash
codeforge ci \
  --changed \
  --baseline .codeforge/baseline.json \
  --fail-on new \
  --sarif codeforge.sarif
```

The comparison reports:

```text
NEW
KNOWN_ACCEPTED
KNOWN_FALSE_POSITIVE
HUMAN_REVIEW
RESOLVED
STALE_BASELINE
AMBIGUOUS_BASELINE_MATCH
```

Default behavior fails only on new regressions. `--fail-on any` is available
for repositories that want all findings to fail. CI never accepts findings,
updates the baseline, or prunes stale entries.

`--new-only` limits SARIF output to new regressions. Without it, accepted
findings remain visible and carry baseline disposition in SARIF properties.
