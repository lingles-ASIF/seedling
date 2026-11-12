# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Release sections are drafted by `python3 tools/exec_summary.py --changelog`
from conventional commit messages.

## [Unreleased]

### Added

- Growth engine in Rust (`#![no_std]`, zero dependencies) compiled to
  WebAssembly: a shoot that sprouts and curves after the pointer, sets leaves
  and the odd flower, blooms when the pointer rests, retracts when the pointer
  slides back along the stem, and withers back to the seed when the pointer
  leaves.
- Single-file page `seedling.html` that runs offline from disk; `build.py`
  embeds the WebAssembly and generates a hash-pinned Content-Security-Policy.
- Security review (`security/REPORT.md`), fuzz harness (`security/fuzz.mjs`)
  and headless-browser policy checks (`security/browser_check.py`).
- Governance: git hooks and a deterministic audit log, the scored security
  gate (`security/gate.py`) with cited sources, CI / release / tracker
  workflows, and the dual MIT OR Apache-2.0 license with NOTICE and AUTHORS.

### Security

- `tick` before `init` no longer underflows an index (was a trap).
- NaN, Infinity and overflowing coordinates are sanitized at the boundary and
  can no longer corrupt state permanently.
- Integer overflow checks enabled in release builds; `panic = "abort"`.
- The page denies everything by default (`default-src 'none'`,
  `connect-src 'none'`) and allows only its own hashed script and style.
