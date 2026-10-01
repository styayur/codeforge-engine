# Benchmarking

Benchmarks measure external commands or engine operations with real wall-clock samples. No estimated data is written.

## Built-in suite

`cargo bench -p codeforge-core --bench runtime` covers:

- workspace startup
- repository indexing
- single-file Tree-sitter analysis
- incremental review
- AST search
- diagnostic aggregation
- a 1,000-file synthetic repository

## Project benchmark command

Configure `.codeforge.toml` or let CodeForge detect Cargo, Go, or package `bench` scripts. The runner performs one warm-up and the requested number of measured samples, then reports mean, median, variance, min, and max.

## Reporting rules

- Do not display percentage improvements without before/after measurements.
- Include the command and environment with published results.
- Distinguish heuristic candidates from measured optimization.
