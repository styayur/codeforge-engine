# Documentation

Start with the [project README](../README.md) for installation and a first scan.
v1.1.0 is the current stable release; Experimental capabilities are labeled
explicitly and remain opt-in.

| Question | Guide |
|---|---|
| How does the runtime work? | [Architecture](ARCHITECTURE.md) |
| What can each language analyze? | [Engines](ENGINES.md), [rule IDs](rule-ids.md), [tool adapters](tool-adapters.md) |
| What does a passing result establish? | [Evidence model](EVIDENCE_MODEL.md), [verification](VERIFICATION.md) |
| How do I control known findings? | [Baseline](baseline.md), [regression CI](regression-ci.md), [finding disposition](finding-disposition.md) |
| How do I automate repositories? | [Fleet](FLEET.md), [PR mode](pr-mode.md), [transformation safety](transformation-safety.md) |
| How do I inspect context or external paths? | [Source context](source-context.md), [SARIF](SARIF.md), [external analysis](external-analysis.md) |
| What are the trust boundaries? | [Security](SECURITY.md), [plugins](PLUGINS.md) |
| How do I reproduce measurements? | [Benchmarking](benchmarking.md) |
| How do I contribute or prepare a release? | [Development](DEVELOPMENT.md), [release preparation](RELEASE_PREPARATION.md), [roadmap](ROADMAP.md) |

The [protocol reference](protocol.md) documents contracts and [doctor](doctor.md)
explains local tool inspection. Historical reports and earlier documentation
are retained in the [archive](archive/README.md), not used as current promises.
