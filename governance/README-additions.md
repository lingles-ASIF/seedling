# Proposed README additions

Text for the main development session to merge into `README.md` (this file is
not meant to be read on its own). Suggested placement: after "Knobs worth
turning".

---

## Security and governance

Every change passes a scored security gate before it can be published:

```
python3 security/gate.py --full     # Clippy x4, unsafe audit, wasm imports, supply chain,
                                    # release profile, 1M-frame fuzz, browser + CSP probes
```

The gate must score at least 95 with no critical failure. Git hooks
(`sh tools/install-hooks.sh`) run a fast version before each commit that
touches code and keep an audit log of every change in `governance/audit/`.
GitHub Actions run the full gate on every push and pull request, publish
releases to crates.io with Trusted Publishing, and keep a daily status page
in the pinned "Project status & tracker" issue. See `governance/README.md` for the full map,
`SECURITY.md` to report a vulnerability privately, and `CONTRIBUTING.md` for
commit conventions.

## Authorship & license

Copyright (c) 2026 Scott Lingley

Scott Lingley conceived and directed Seedling and holds its intellectual property.
Implementation, testing and documentation were produced with substantial
assistance from Claude Opus 5.5 (`claude-opus-5-5`), an AI model by
Anthropic, under Scott Lingley's direction. Commits made with that assistance carry
a `Co-Authored-By: Claude Opus 5.5` trailer; see `AUTHORS.md`.

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in Seedling
by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions. See `NOTICE`.

Legal note: whether and how copyright attaches to AI-assisted work varies by jurisdiction; this notice states the author's claim and is not legal advice.

## Suggested table rows for "What's where"

| File | Role |
|---|---|
| `security/gate.py` | The security gate: scored checks, reports in `security/reports/`, sources in `security/WORKS_CITED.md`. |
| `governance/` | Governance map (`README.md`), settings (`config.toml`), audit log, tracker. |
| `tools/` | Git hooks, audit log, executive summary, tracker, release checks. |
| `.github/` | CI, release and tracker workflows; issue and PR templates. |
| `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `AUTHORS.md` | Dual license and authorship. |

## Suggested Cargo.toml additions (needed before publishing to crates.io)

```toml
[package]
license = "MIT OR Apache-2.0"
repository = "https://github.com/lingles-ASIF/seedling"   # after the repo exists
readme = "README.md"

[package.metadata.docs.rs]
targets = ["wasm32-unknown-unknown"]   # the engine uses core::arch::wasm32
```
