# Authors

## Owner and author

**Scott L.** conceived and directed the Seedling project and holds its
intellectual property. Copyright (c) 2026 Scott L., licensed under
MIT OR Apache-2.0 at your option (see `NOTICE`, `LICENSE-MIT`,
`LICENSE-APACHE`).

## AI assistance

Implementation, testing and documentation were produced with substantial
assistance from **Claude Opus 5.5** (model ID `claude-opus-5-5`), an AI model
by Anthropic, under the direction of Scott L. Claude is a tool used in the
work, not an owner of it.

> Legal note: whether and how copyright attaches to AI-assisted work varies by jurisdiction; this notice states the author's claim and is not legal advice.

## Reading the git history

Every commit made in the development environment ends with two trailer lines
(added automatically by the `commit-msg` hook, see `CONTRIBUTING.md`):

```
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01P3DGbhAvhQ3fgCmzajy6ah
```

- **Author** (`git log --format='%an'`): the person responsible for the
  commit. For this project that is Scott L.
- **Co-Authored-By**: the commit was produced with Claude's assistance.
  GitHub shows this as a co-author on the commit page.
- **Claude-Session**: the working session in which the change was made, so a
  change can be traced back to the conversation that directed it.
- **Security-Gate** (when present): the result of the pre-commit security gate
  for exactly the files in that commit, e.g. `100.0/95 PASS (fast, tree ...)`,
  or `BYPASSED (SEEDLING_SKIP_GATE=1)`.

Useful commands:

```
git log --format='%h %an | %(trailers:key=Co-Authored-By,valueonly,separator=%x2C )| %s'
git log --format='%h %(trailers:key=Security-Gate,valueonly)%s'
python3 tools/audit.py summary --since <ref>
```

`governance/audit/AUDIT_LOG.md` lists every commit with its author,
co-author, files changed and security-gate result, regenerated from git.
