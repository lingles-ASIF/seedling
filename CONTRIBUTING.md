# Contributing to Seedling

Thanks for helping. Three rules keep the project auditable:

1. **Conventional commits.** Subjects look like `type(scope): summary`.
2. **The security gate must pass** (score >= 95, no critical failure).
3. **Every change is traceable** via the audit log and commit trailers.

## Setup

```
rustup target add wasm32-unknown-unknown
rustup component add clippy
sh tools/install-hooks.sh          # once per clone; needs Python 3.11+
pip install playwright && python -m playwright install chromium   # for --full
```

## Commit messages

[Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/):

| Type | Use for | Shows in release notes as |
|---|---|---|
| `feat` | new behaviour users can see | Added / "New" |
| `fix` | bug fixes | Fixed |
| `perf` | faster, same behaviour | Changed / "Faster" |
| `security` (or scope `security`) | hardening, vulnerability fixes | Security |
| `refactor`, `docs`, `test`, `build`, `ci`, `chore`, `style` | internal | "Behind the scenes" |

Add `!` after the type (`feat!: ...`) or a `BREAKING CHANGE:` footer for
breaking changes. `tools/exec_summary.py` turns these into the plain-English
executive summary and the CHANGELOG section of each release.

## What the hooks do

`tools/install-hooks.sh` sets `core.hooksPath` to the tracked `tools/hooks/`.

- **pre-commit**: if any staged path matches `[gate].trigger_globs` in
  `governance/config.toml` (Rust sources, any `Cargo.toml`, `Cargo.lock`,
  `build.py`, `web/`, the fuzz/browser harnesses, the gate itself), runs
  `security/gate.py --hook` (fast mode: build, Clippy x4, unsafe audit, wasm
  imports, supply chain, profile, CSP, 50,000-frame fuzz). A failure blocks
  the commit. Emergency bypass: `SEEDLING_SKIP_GATE=1 git commit ...`; the
  bypass is recorded in a `Security-Gate: BYPASSED` trailer and in
  `.git/seedling-gate-bypass.jsonl`, and CI still runs the full gate.
- **commit-msg**: adds a `Security-Gate:` trailer with the pre-commit result,
  then ensures the message ends with the configured `Co-Authored-By` and
  `Claude-Session` trailers (`[audit].trailers`).
- **post-commit**: runs `python3 tools/audit.py rebuild`, regenerating
  `governance/audit/AUDIT_LOG.md` and `audit_log.jsonl` from git history. It
  never commits; the refreshed files go into your next commit.

The gate tests the working tree, so stage everything you mean to commit
before committing. Sandboxes without a downloadable wasm32 standard library
can set `SEEDLING_BUILD_STD=1` (the installer does this automatically when
needed).

## Running the gate yourself

```
python3 security/gate.py --full     # what CI and releases run
python3 security/gate.py --fast     # what the hook runs
```

Reports land in `security/reports/latest.{md,json}`.

## Pull requests

Fill in the PR template checklist. CI posts the gate score table as a single
comment on the PR and updates it on every push. Change requests from users
are issues labelled `change-request`; link them from the PR.

## Licensing of contributions

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in Seedling by you, as defined in the Apache-2.0
license, shall be dual licensed as MIT OR Apache-2.0, without any additional
terms or conditions.
