## Summary

<!-- What changes for users, in one or two sentences. Link change requests: "Closes #123". -->

## Type

<!-- Conventional commit type of the squash/merge commit: feat, fix, perf, security, refactor, docs, test, build, ci, chore. Add ! if breaking. -->

## Audit checklist

- [ ] Commit subjects follow Conventional Commits (`type(scope): summary`).
- [ ] `python3 security/gate.py --full` passes locally (score >= 95, no critical failure), or CI's sticky comment shows PASS.
- [ ] No `SEEDLING_SKIP_GATE=1` bypass in these commits, or the reason is explained below.
- [ ] Every new `unsafe` block has a `// SAFETY:` comment; any new `unsafe fn` has a `# Safety` doc section.
- [ ] No new dependencies, or each is justified below and `cargo audit` / `cargo deny check` are clean.
- [ ] Content-Security-Policy unchanged, or the change is explained (no `'unsafe-inline'` / `'unsafe-eval'`).
- [ ] `seedling.html` rebuilt with `python3 build.py` if Rust or `web/` changed.
- [ ] CHANGELOG.md `[Unreleased]` updated for user-visible changes.
- [ ] Commits carry the `Co-Authored-By` / `Claude-Session` trailers where AI assistance was used (the hooks add them).
- [ ] Hooks installed (`sh tools/install-hooks.sh`) so the audit log in `governance/audit/` is current.

## Risks and notes

<!-- Anything a reviewer should look at closely; reasons for any unchecked box. -->
