# On-demand context and impact suggestions

Experimental and included in the v1.0.0 package; never part of the default scan path.

```bash
codeforge context . --query verification --limit 10 --max-bytes 32768
codeforge impact . --depth 4 --graph
```

Both commands print JSON, run locally, and read source without applying changes,
starting analyzer tools or calling a model. Impact requires and invokes local
Git to inspect changes. They reuse WorkspaceManager ignore rules,
existing Tree-sitter grammars and Git status (staged, unstaged and untracked).
They are not invoked by scan/review/Fleet or by the verification gate.

Context uses case-sensitive literal symbol-name matching. Each fragment carries
the relative source path, content hash, original symbol range, exact excerpt
range and a truncation flag. Line and character-column positions are one-based;
byte offsets are zero-based. A changed source hash invalidates the excerpt.
The entire workspace is enumerated using the existing scanner; parsing defaults
to 512 sorted files, with a hard maximum of 8192 files, 1 MiB per source, 20,000
symbols and 20,000 relations. Output is at most 100 fragments/256 KiB of source.
`partial` reports omitted, unsupported, invalid or changed source and bounds.

The graph contains function/method declarations and candidate call edges found
by Tree-sitter. A matching callee name is **heuristic**, including when unique.
All same-name targets are retained and ambiguous edges are labeled. Imports,
scope/type resolution, callbacks, generated code, macros, dynamic dispatch and
deleted symbols are not resolved. Cycles are visited once and traversal depth
is bounded. Test naming/attributes supply suggestions, not a test selection
guarantee. The full configured test/build/semantic gates are unchanged.

These commands do not provide whole-program static analysis or semantic
verification. They expose evidence for agent inspection; they never convert a
suggested relation or test into permission to apply a fix.
