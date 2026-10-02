# Rule IDs

Rule IDs are stable public identifiers. Existing pre-v0.2 IDs remain valid for
backward compatibility. New rules use the `CF-` namespace.

```text
CF-GEN-001   generic tool-adapter finding
CF-MD-001    broken Markdown reference
CF-JSON-001  invalid JSON
CF-YAML-001  tab indentation in YAML
CF-TOML-001  duplicate TOML key
CF-HTML-001  missing standalone HTML root
CF-CSS-001   unbalanced CSS braces
CF-DART-001  Dart print statement review
CF-DART-002  Dart TODO marker
CF-DART-003  Dart dynamic type review
CF-PS-001    PowerShell Invoke-Expression review
CF-PS-002    PowerShell secure-string review
CF-PS-003    PowerShell Write-Host review
CF-FLEET-001 reserved for fleet-level policy findings
```

Rule IDs must not change silently. New behavior for an existing rule requires a
test and a changelog entry. Transformation proposals always carry a
transformation class in addition to the rule ID, so a rule can evolve from a
review-only finding to a safe fix without weakening the safety model.
