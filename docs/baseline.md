# Reviewed Baseline

CodeForge does not treat zero findings as the goal. A reviewed baseline lets CI
separate known debt from new regressions while keeping every accepted finding
visible.

## File

The default baseline is `.codeforge/baseline.json`:

```json
{
  "schema_version": 1,
  "codeforge_version": "0.2.1",
  "entries": [
    {
      "fingerprint": "sha256...",
      "structural_fingerprint": "sha256...",
      "context_fingerprint": "sha256...",
      "rule_id": "RS-CORRECTNESS-001",
      "path": "src/example.rs",
      "symbol": "read_value",
      "context_hash": "sha256...",
      "disposition": "accepted",
      "reason": "Documented invariant makes the panic unreachable.",
      "first_seen": "2026-10-03T00:00:00Z",
      "last_reviewed": "2026-10-03T00:00:00Z",
      "rule_version": "0.2.1",
      "lifecycle": "active"
    }
  ]
}
```

Paths are repository-relative. Baselines must not contain home paths,
usernames, machine identifiers, secrets, or environment values.

## Fingerprints

Fingerprints use layered identity:

1. `EXACT`: rule, relative path, symbol/message, and nearby context hash.
2. `STRUCTURAL`: rule, symbol, and normalized message; survives line shifts and
   reliable file renames.
3. `CONTEXT_RELOCATED`: rule, normalized message, and nearby context hash.

If more than one baseline entry can match, CodeForge returns
`AMBIGUOUS_BASELINE_MATCH` and refuses to auto-suppress.

## Lifecycle

- `ACTIVE`: matched by the current finding set.
- `STALE`: rule version changed and suppression must be reviewed.
- `RESOLVED`: the finding no longer appears.
- `AMBIGUOUS`: multiple candidates prevent a safe match.

## Commands

```bash
codeforge baseline show --file .codeforge/baseline.json
codeforge baseline review --path ./repo --file .codeforge/baseline.json
codeforge baseline review --path ./repo --file .codeforge/baseline.json --include-fixtures
codeforge baseline accept --path ./repo --file .codeforge/baseline.json --finding <fingerprint> --reason "..."
codeforge baseline false-positive --path ./repo --file .codeforge/baseline.json --finding <fingerprint> --reason "..."
codeforge baseline unreview --path ./repo --file .codeforge/baseline.json --finding <fingerprint>
codeforge baseline prune --file .codeforge/baseline.json
codeforge baseline migrate --from legacy.json --to .codeforge/baseline.json
```

Baseline writes are explicit. Audit, review, and CI are read-only with respect
to the baseline.
