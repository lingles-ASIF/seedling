# Seedling

A seed pulses just under a hinted crust. Move the pointer and a shoot sprouts,
curving after it, setting leaves and the odd small flower along the way. Rest
the pointer and the tip blooms. Slide back along the fresh stem and it
shrinks after you. Leave the window, or return to the seed, and it withers
back underground to wait.

**To run:** double-click `seedling.html`. No server, no internet, no install.

## What's where

| File | Role |
|---|---|
| `src/lib.rs` | The growth engine, in Rust (compiled to WebAssembly). All behavior lives here. |
| `web/template.html` | A thin page: forwards pointer + clock to Rust, paints the shapes Rust returns. |
| `build.py` | Compiles the Rust and embeds the `.wasm` into one self-contained HTML file. |
| `seedling.html` | The built result. |
| `security/` | Security review (`REPORT.md`), fuzz harness and browser policy checks. |
| `security/gate.py` | The security gate: scored checks, reports in `security/reports/`, sources in `security/WORKS_CITED.md`. |
| `governance/` | Governance map (`README.md`), settings (`config.toml`), audit log, tracker. |
| `tools/` | Git hooks, audit log, executive summary, tracker, release checks. |
| `.github/` | CI, release and tracker workflows; issue and PR templates. |
| `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `AUTHORS.md` | Dual license and authorship. |

## Rebuilding after you change the Rust

```
rustup target add wasm32-unknown-unknown   # one time
python3 build.py
```

## Knobs worth turning (top of `src/lib.rs`)

- `MAX_TURN` - tighter or looser curves (lower = more graceful, wider arcs)
- `WANDER_KICK` / `WANDER_DAMP` - how much the stem meanders on its own
- `FOG` / `FOG_REACH` - how faint the remembered stem gets, and how far behind
  the tip it stays vivid
- `GROW_MIN` / `GROW_MAX` - growth speed
- In `add_twig`: `flower: self.rnd() < 0.2` - fraction of twigs ending in a flower

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
in `governance/TRACKER.md`. See `governance/README.md` for the full map,
`SECURITY.md` to report a vulnerability privately, and `CONTRIBUTING.md` for
commit conventions.

## Authorship & license

Copyright (c) 2026 Scott L.

Scott L. conceived and directed Seedling and holds its intellectual property.
Implementation, testing and documentation were produced with substantial
assistance from Claude Opus 5.5 (`claude-opus-5-5`), an AI model by
Anthropic, under Scott L.'s direction. Commits made with that assistance carry
a `Co-Authored-By: Claude Opus 5.5` trailer; see `AUTHORS.md`.

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in Seedling
by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions. See `NOTICE`.

Legal note: whether and how copyright attaches to AI-assisted work varies by jurisdiction; this notice states the author's claim and is not legal advice.
