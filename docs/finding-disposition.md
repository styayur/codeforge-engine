# Finding Disposition

Every reviewed finding has one source disposition:

- `unreviewed`
- `accepted`
- `false_positive`
- `human_review`
- `fixed`

`accepted` and `false_positive` require a non-empty reason. `human_review` is
used when the evidence is insufficient for an automatic or deterministic
decision.

Tool gaps are not finding dispositions. A missing analyzer, formatter, SDK, or
verification command is execution capability state and is reported separately.

## Provenance

Findings also carry optional producer metadata:

```text
producer
producer_version
native_rule_id
codeforge_rule_id
source_context
confidence
rule_version
```

This keeps compiler-backed findings distinct from CodeForge heuristics.

## Source context

Source context is classified as one of:

```text
production
test
benchmark
fixture
generated
build_script
config
vendor
```

Rules can lower severity or confidence for non-production context without
blanket-suppressing all test or fixture code.
