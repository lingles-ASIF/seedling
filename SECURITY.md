# Security policy

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Report privately through GitHub's private vulnerability reporting: open the
repository's **Security** tab, then **Advisories > Report a vulnerability**.
Only the maintainer sees the report. Please include:

- what an attacker can do, and the input or steps that trigger it;
- the version or commit (`git rev-parse HEAD`) and browser, if relevant;
- a proof of concept if you have one (a failing `security/fuzz.mjs` seed is ideal).

You can expect an acknowledgement within 7 days and a status update within
30 days. Fixes are released as a patch version and credited in the advisory
unless you prefer otherwise.

## Supported versions

Seedling is pre-1.0. Only the latest released version receives security fixes.

| Version | Supported |
|---|---|
| latest `0.x` release | yes |
| older releases | no, please upgrade |

## What protects users today

- **Engine** (`src/`): Rust, `#![no_std]`, no allocator, zero dependencies,
  integer overflow checks on in release, `panic = "abort"`, and every
  `unsafe` block justified with a `// SAFETY:` comment.
- **WebAssembly module**: imports nothing from the page, so it cannot reach
  the DOM, network or files.
- **Page** (`seedling.html`): hash-pinned Content-Security-Policy with
  `default-src 'none'` and `connect-src 'none'`; no `'unsafe-inline'` or
  `'unsafe-eval'` (only `'wasm-unsafe-eval'`, which permits WebAssembly
  compilation and nothing else).

## The security gate

Nothing is published unless `security/gate.py` passes: a weighted score of at
least **95** (configurable in `governance/config.toml`) and no failed critical
check. It runs:

- before commits that touch Rust, web or build files (fast mode, git hook);
- on every push and pull request (full mode, `.github/workflows/ci.yml`);
- before every crates.io release (full mode, `.github/workflows/release.yml`).

Checks: Clippy (default, pedantic, nursery, `undocumented_unsafe_blocks`),
`SAFETY:` comments on every `unsafe`, wasm build and empty import list,
supply chain (zero dependencies, or `cargo audit` / `cargo deny`), release
profile hardening, a 1,000,000-frame fuzz of every export, headless-browser
behaviour and CSP probes, the CSP itself, and repository hygiene files. Each
report cites its sources; see `security/WORKS_CITED.md`. The original
hand-written review is `security/REPORT.md`.

The score is a project-internal measure, not an industry rating or a
third-party certification.

Releases are published from GitHub Actions with crates.io Trusted Publishing
(short-lived OIDC tokens; no long-lived API token is stored in the repository).
