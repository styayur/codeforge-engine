# Verification

The verification pipeline records what ran and what passed. Its configurable
stages include syntax, typecheck, build, tests, fuzz, differential, equivalence
and benchmark checks. They are capabilities of the runner, not a promise that
every project has every tool or that all stages always execute.

## Configure trusted commands

Place project-specific commands in `.codeforge.toml`:

```toml
[commands.test]
program = "cargo"
args = ["test", "--all-targets"]
cwd = "."
timeout_secs = 300

[commands.benchmark]
program = "cargo"
args = ["bench", "--quiet"]
cwd = "."
timeout_secs = 600
```

Supported keys are `format`, `lint`, `syntax`, `typecheck`, `build`, `test`,
`fuzz`, `differential`, `equivalence` and `benchmark`. Format/lint configuration
takes precedence over corresponding detected syntax/typecheck commands. Native
project detection can supply commands when supported; inspect configuration and
tool status before running unfamiliar projects.

```bash
codeforge doctor .
codeforge verify .
codeforge benchmark . --samples 5
```

These commands can execute installed tools. Builds/tests may modify files or
access the network. The runner uses explicit arguments, timeouts and bounded
output; it inherits the environment and host permissions. See [Security](SECURITY.md).

## Interpret results and application gates

Check each stage's status, command, output and evaluated candidate. `Unavailable`,
`Skipped` and `NotRun` do not establish success. A passing test suite only covers
what that suite checks; a benchmark cannot establish equivalence.

Preview, single-workspace verification and Fleet policy are different paths.
Do not treat `VerificationResult::accepted` as the complete automatic-application
policy. Fleet requires passed syntax plus all policy-required builds/tests and
blocks explicit failures. Review the patch, protected paths and diff budgets as
well. See [Fleet](FLEET.md) and [Transformation Safety](transformation-safety.md).

Optimization evaluates candidates in a temporary workspace before application;
this is a filesystem copy, not an OS sandbox for the commands it runs. Transaction
rollback restores tracked edits managed by the transaction, not arbitrary
external side effects such as network requests.

[Evidence Model](EVIDENCE_MODEL.md) defines the existing labels, including why a
successful configured equivalence command is not automatically a formal proof.
