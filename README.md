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
| `crates/seedling/` | The growth engine: a reusable Rust library (`Plant`, `Config`, `Renderer`). All behaviour lives here. Published to crates.io. |
| `crates/seedling-wasm/` | Thin WebAssembly wrapper the page loads: one `Plant`, four exported functions, a flat draw list. |
| `web/template.html` | A thin page: forwards pointer + clock to Rust, paints the shapes Rust returns. |
| `build.py` | Compiles the Rust and embeds the `.wasm` into one self-contained HTML file. |
| `seedling.html` | The built result. |
| `security/` | Security review (`REPORT.md`), fuzz harness and browser policy checks. |
| `security/gate.py` | The security gate: scored checks, reports in `security/reports/`, sources in `security/WORKS_CITED.md`. |
| `governance/` | Governance map (`README.md`), settings (`config.toml`), audit log, tracker. |
| `tools/` | Git hooks, audit log, executive summary, tracker, release checks. |
| `.github/` | CI, release and tracker workflows; issue and PR templates. |
| `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `AUTHORS.md` | Dual license and authorship. |

## Turning leaves into links

Leaves and twig flowers can be links to other pages. Edit the settings block
at the top of `<body>` in `web/template.html` (or directly in `seedling.html`):

```json
{
  "links": [
    { "href": "about.html", "label": "About us" },
    { "href": "https://example.com/shop", "label": "Shop" }
  ],
  "persist": true,
  "newTab": false
}
```

- **Order:** the first leaf or flower to grow gets the first link, the next
  gets the second, and so on. If a linked branch withers, its link moves to
  the next leaf that grows, so every link stays reachable.
- **Safety:** only `https:`, `http:`, `mailto:` and relative addresses are
  accepted; anything else (such as `javascript:`) is skipped. Labels are shown
  as plain text, never as HTML.
- **Everyone can use them:** linked leaves get a gold ring and a label on
  hover. Keyboard and screen-reader visitors get the same links as a list that
  appears when they press Tab.
- **Persist:** with links, the grown plant stays when the pointer leaves, so
  visitors can come back and click; returning to the seed still resets it.
  Set `"persist": false` for the original wither-away behaviour.
- **While the pointer is on a link, the plant holds still**, so reaching for a
  leaf never makes the branch chase or prune itself.

## Rebuilding after you change the Rust

```
rustup target add wasm32-unknown-unknown   # one time (the pinned toolchain may install it for you)
cargo test -p seedling                     # library tests, run natively
python3 build.py                           # rebuild seedling.html
```

## Using the library in your own Rust project

`crates/seedling` is a standalone library: no `unsafe`, no dependencies, no
allocation, and it runs on any platform. You call `Plant::step` each frame with
the pointer position, and `Plant::draw` hands shapes to your own `Renderer`.
See `crates/seedling/README.md` for an example.

## Settings worth turning (`seedling::Config`)

- `max_turn` - tighter or looser curves (lower = more graceful, wider arcs)
- `wander_kick` / `wander_damp` - how much the stem meanders on its own
- `fog` / `fog_reach` - how faint the remembered stem gets, and how far behind
  the tip it stays vivid
- `grow_min` / `grow_max` - growth speed
- `flower_chance` - share of twigs that end in a flower instead of a leaf
- `links` - how many links to hand out to leaves and flowers (up to 64)
- `persist` - keep the grown plant when the pointer leaves

Every value is clamped into a safe range, so no setting can break the plant.

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
releases to crates.io with Trusted Publishing, and keep a daily status report
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

<!-- Uncomment this section once https://github.com/sponsors/lingles-ASIF is live
     (and the github: line in .github/FUNDING.yml).

## Support Seedling

Seedling is free and open source. It is built with substantial help from
Claude, Anthropic's AI model, which has real usage costs. If Seedling is useful
to you, a sponsorship helps cover those costs and keeps development going:
[Sponsor on GitHub](https://github.com/sponsors/lingles-ASIF).

Sponsorships go to the maintainer personally. They are gifts, not
tax-deductible charitable donations, and they don't buy support guarantees.
-->
