Rust RS-PERFORMANCE-001 previously searched entire binary expressions for text,
so unrelated `.len()` and `== 0` operands could produce a destructive replacement.
The rule now checks immediate Tree-sitter operands, a zero-argument len method
and the exact zero literal. Suggestions carry medium heuristic confidence and
are not marked safe, since syntax alone cannot establish custom receiver types
or equivalent method behavior. Build/test gates do not prove semantic equivalence.

Regression coverage includes boolean/arithmetic nesting, nonzero literal prefixes,
argument-bearing calls and an exact comparison inside a compound condition.
No dependencies, rule IDs, Baseline format or transaction APIs change.
