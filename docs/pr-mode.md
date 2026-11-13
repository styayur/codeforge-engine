# PR Mode

PR creation is explicit opt-in through `--open-pr`. Default fleet
transformation commands only create a preview and evidence bundle.

PR mode enforces:

- one repository per branch;
- one repository per pull request;
- no force push or history rewrite;
- clean-tree policy before apply;
- verification and diff-budget checks before commit;
- optional GitHub integration through the local `gh` CLI.

If `gh` is unavailable, the transformation is not treated as failed. CodeForge
reports `PR creation unavailable` and keeps the branch, transaction, and report.

PR titles default to:

```text
chore(codeforge): safe refactoring pass
```

or:

```text
refactor: verified structural cleanup
```

The body contains the summary, CodeForge version, transformation classes, files
changed, verification, benchmark status, risk, and report path.
