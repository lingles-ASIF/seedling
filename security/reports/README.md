# Security gate reports

`python3 security/gate.py` writes here:

- `latest.json` / `latest.md`: the most recent run.
- `history/<commit-sha>[-dirty].{json,md}`: a copy named by the commit that was tested.
- `history/tree-<tree-sha>.{json,md}`: pre-commit runs, named by the git tree being committed
  (the commit-msg hook reads this to write the `Security-Gate:` trailer).

These files are generated and git-ignored. The durable records are the
`Security-Gate:` commit trailers (collected into `governance/audit/`), the CI
artifact `security-reports` on every push/PR, and the report attached to each
GitHub Release.
