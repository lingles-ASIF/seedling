# Seedling audit log

Every commit in this repository, oldest first, regenerated deterministically from `git log` by `python3 tools/audit.py rebuild` (run automatically by the post-commit hook). Do not edit by hand. Timestamps are commit times in America/New_York. Because a hook may not create commits, this file is always complete up to the commit *before* the one that contains it.

| # | When | Commit | Subject | Files | +/- | Security gate |
|---:|---|---|---|---:|---|---|
| 1 | 2026-11-12 10:26:45 EST | `f9b307f859` | Baseline: Seedling growth engine, page, build script and security review | 11 | +1594/-0 | no gate record |
| 2 | 2026-11-12 10:43:12 EST | `fd3b8f8ec9` | docs(legal): dual MIT OR Apache-2.0 license, NOTICE and AUTHORS | 4 | +175/-0 | not run (no gated paths changed) |
| 3 | 2026-11-12 10:43:12 EST | `f9e42ee2b8` | security: scored security gate with cited sources | 7 | +964/-0 | no gate record |
| 4 | 2026-11-12 10:43:12 EST | `bbddacc381` | build(hooks): local change monitor and deterministic audit log | 6 | +389/-0 | not run (no gated paths changed) |
| 5 | 2026-11-12 10:43:19 EST | `09ef3154f6` | ci: GitHub change manager (CI, release, tracker) and project templates | 17 | +1185/-0 | not run (no gated paths changed) |
| 6 | 2026-11-12 10:43:29 EST | `13d37642d8` | docs(governance): one-page governance map and README additions | 9 | +275/-0 | 100.0/95 PASS (fast, tree 2755fd60e7) |
| 7 | 2026-11-12 10:43:47 EST | `9074b441c3` | chore(audit): record audit log through the governance commits | 2 | +21/-0 | not run (no gated paths changed) |
| 8 | 2026-11-12 10:44:06 EST | `f7957d3cf6` | docs(legal): keep addresses out of NOTICE and AUTHORS | 4 | +21/-4 | not run (no gated paths changed) |
| 9 | 2026-11-12 10:45:30 EST | `cd605045f6` | docs: merge governance and authorship sections; add crates.io metadata | 5 | +66/-1 | 100.0/95 PASS (fast, tree 8c4d778fae) |

## Entries

### 2026-11-12 10:26:45 EST `f9b307f859` Baseline: Seedling growth engine, page, build script and security review

- Commit: `f9b307f8597b31025c71c64f3f2d93e235cdebd7`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: no gate record
- Changes: 11 file(s), +1594 / -0

  | File | + | - |
  |---|---:|---:|
  | `.gitignore` | 3 | 0 |
  | `Cargo.lock` | 7 | 0 |
  | `Cargo.toml` | 20 | 0 |
  | `README.md` | 35 | 0 |
  | `build.py` | 87 | 0 |
  | `security/REPORT.md` | 75 | 0 |
  | `security/browser_check.py` | 67 | 0 |
  | `security/fuzz.mjs` | 122 | 0 |
  | `seedling.html` | 208 | 0 |
  | `src/lib.rs` | 762 | 0 |
  | `web/template.html` | 208 | 0 |

### 2026-11-12 10:43:12 EST `fd3b8f8ec9` docs(legal): dual MIT OR Apache-2.0 license, NOTICE and AUTHORS

- Commit: `fd3b8f8ec97d4a947003044a6801a7117149b0c5`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: not run (no gated paths changed)
- Changes: 4 file(s), +175 / -0

  | File | + | - |
  |---|---:|---:|
  | `AUTHORS.md` | 48 | 0 |
  | `LICENSE-APACHE` | 73 | 0 |
  | `LICENSE-MIT` | 18 | 0 |
  | `NOTICE` | 36 | 0 |

### 2026-11-12 10:43:12 EST `f9e42ee2b8` security: scored security gate with cited sources

- Commit: `f9e42ee2b8c347928a497798441c15b37e2e87a8`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: no gate record
- Changes: 7 file(s), +964 / -0

  | File | + | - |
  |---|---:|---:|
  | `SECURITY.md` | 62 | 0 |
  | `governance/config.toml` | 97 | 0 |
  | `security/WORKS_CITED.md` | 45 | 0 |
  | `security/gate.py` | 602 | 0 |
  | `security/reports/.gitignore` | 6 | 0 |
  | `security/reports/README.md` | 13 | 0 |
  | `tools/seedling_config.py` | 139 | 0 |

### 2026-11-12 10:43:12 EST `bbddacc381` build(hooks): local change monitor and deterministic audit log

- Commit: `bbddacc381a8d4cc79ad2c18c0d2f1576b33463f`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: not run (no gated paths changed)
- Changes: 6 file(s), +389 / -0

  | File | + | - |
  |---|---:|---:|
  | `tools/audit.py` | 177 | 0 |
  | `tools/githooks.py` | 137 | 0 |
  | `tools/hooks/commit-msg` | 10 | 0 |
  | `tools/hooks/post-commit` | 10 | 0 |
  | `tools/hooks/pre-commit` | 10 | 0 |
  | `tools/install-hooks.sh` | 45 | 0 |

### 2026-11-12 10:43:19 EST `09ef3154f6` ci: GitHub change manager (CI, release, tracker) and project templates

- Commit: `09ef3154f66d01e47965854401fd304be0748a53`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: not run (no gated paths changed)
- Changes: 17 file(s), +1185 / -0

  | File | + | - |
  |---|---:|---:|
  | `.github/CODEOWNERS` | 6 | 0 |
  | `.github/ISSUE_TEMPLATE/bug_report.yml` | 37 | 0 |
  | `.github/ISSUE_TEMPLATE/change_request.yml` | 45 | 0 |
  | `.github/ISSUE_TEMPLATE/config.yml` | 8 | 0 |
  | `.github/ISSUE_TEMPLATE/security_report.yml` | 28 | 0 |
  | `.github/actions/setup/action.yml` | 37 | 0 |
  | `.github/dependabot.yml` | 19 | 0 |
  | `.github/pull_request_template.md` | 24 | 0 |
  | `.github/workflows/ci.yml` | 106 | 0 |
  | `.github/workflows/release.yml` | 156 | 0 |
  | `.github/workflows/tracker.yml` | 50 | 0 |
  | `CHANGELOG.md` | 34 | 0 |
  | `CONTRIBUTING.md` | 78 | 0 |
  | `tools/exec_summary.py` | 187 | 0 |
  | `tools/fixtures/tracker_fixture.json` | 24 | 0 |
  | `tools/release_check.py` | 53 | 0 |
  | `tools/tracker.py` | 293 | 0 |

### 2026-11-12 10:43:29 EST `13d37642d8` docs(governance): one-page governance map and README additions

- Commit: `13d37642d8d1a01e40ddc5dbc9b029aa8f9183fc`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: 100.0/95 PASS (fast, tree 2755fd60e7)
- Changes: 9 file(s), +275 / -0

  | File | + | - |
  |---|---:|---:|
  | `governance/README-additions.md` | 65 | 0 |
  | `governance/README.md` | 97 | 0 |
  | `governance/audit/AUDIT_LOG.md` | 108 | 0 |
  | `governance/audit/audit_log.jsonl` | 5 | 0 |
  | `security/gate.py` | 0 | 0 |
  | `tools/audit.py` | 0 | 0 |
  | `tools/githooks.py` | 0 | 0 |
  | `tools/install-hooks.sh` | 0 | 0 |
  | `tools/seedling_config.py` | 0 | 0 |

### 2026-11-12 10:43:47 EST `9074b441c3` chore(audit): record audit log through the governance commits

- Commit: `9074b441c3736ddc8366d22e524c7006f5624161`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: not run (no gated paths changed)
- Changes: 2 file(s), +21 / -0

  | File | + | - |
  |---|---:|---:|
  | `governance/audit/AUDIT_LOG.md` | 20 | 0 |
  | `governance/audit/audit_log.jsonl` | 1 | 0 |

### 2026-11-12 10:44:06 EST `f7957d3cf6` docs(legal): keep addresses out of NOTICE and AUTHORS

- Commit: `f7957d3cf6b2e432fa17ea8d50fb2e5b0afa365c`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: not run (no gated paths changed)
- Changes: 4 file(s), +21 / -4

  | File | + | - |
  |---|---:|---:|
  | `AUTHORS.md` | 5 | 1 |
  | `NOTICE` | 2 | 3 |
  | `governance/audit/AUDIT_LOG.md` | 13 | 0 |
  | `governance/audit/audit_log.jsonl` | 1 | 0 |

### 2026-11-12 10:45:30 EST `cd605045f6` docs: merge governance and authorship sections; add crates.io metadata

- Commit: `cd605045f6128175b475140074e123389f6b2224`
- Author: Scott L.; co-author(s): Claude Opus 5.5
- Security gate: 100.0/95 PASS (fast, tree 8c4d778fae)
- Changes: 5 file(s), +66 / -1

  | File | + | - |
  |---|---:|---:|
  | `Cargo.toml` | 8 | 0 |
  | `README.md` | 41 | 0 |
  | `governance/audit/AUDIT_LOG.md` | 15 | 0 |
  | `governance/audit/audit_log.jsonl` | 1 | 0 |
  | `security/REPORT.md` | 1 | 1 |
