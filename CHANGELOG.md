# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Release sections are drafted by `python3 tools/exec_summary.py --changelog`
from conventional commit messages.

## [Unreleased]

### Added

- Circuit-board birch tree (`Style::CircuitBirch`, optional Cargo feature
  `circuit`): a straight trunk rises toward the pointer; branches leave it at
  45° and route toward the pointer like circuit traces (straight runs, 45°
  turns only), fork 45° side traces, and end in birch leaves that can carry
  links. Branches left alone fade into the fog. New settings: `branch_length`,
  `trunk_height`, `branch_spacing`; `StrokeKind::{Trunk, Trace}`,
  `LeafKind::{Vine, Birch}`. Page setting `"style": "circuit-birch"`, and a
  ready-made `demos/circuit-birch.html`.
- Leaf links. Leaves and twig flowers can carry links, handed out in growth
  order and recycled when a linked branch withers (`Config::links`,
  `Leaf::link`, `Flower::link`). The page reads links from a JSON settings
  block, accepts only `https:`, `http:`, `mailto:` and relative addresses,
  shows labels as text on hover, marks linked leaves with a gold ring, and
  offers the same links as a keyboard/screen-reader list.
- `Config::persist`: keep the grown plant when the pointer leaves (on by
  default in the page when links are set).
- `Plant::rest`: hold still without growing or withering; the page uses it
  while the pointer is over a link.
- Security gate: module-size limit (`max_wasm_bytes`), link checks in the fuzz
  harness (valid, unique, actually handed out), and browser probes for links,
  hostile link settings, keyboard access, persistence and broken settings.
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

### Changed

- Split into a Cargo workspace. `crates/seedling` is now a reusable,
  platform-independent library with a public `Plant` / `Config` / `Renderer`
  API, `#![forbid(unsafe_code)]`, no dependencies and no allocation, plus 13
  unit tests and a doc test that run natively. `crates/seedling-wasm` is a
  thin browser wrapper (`publish = false`) that keeps the exact same four
  exports and draw-list format. Output was verified bit-for-bit identical to
  the previous engine across 60,000 frames, including hostile inputs.
- The security gate runs the library's tests as a new critical check
  (`tests.unit`).

- The daily tracker no longer commits to `main`. Its report goes to the
  pinned "Project status & tracker" issue and is attached to each run as
  `TRACKER.md` and `tracker.json`; the workflow's token is now read-only.
  `main` is protected by a ruleset that only accepts pull requests that pass
  the security gate.

### Security

- `tick` before `init` no longer underflows an index (was a trap).
- NaN, Infinity and overflowing coordinates are sanitized at the boundary and
  can no longer corrupt state permanently.
- Integer overflow checks enabled in release builds; `panic = "abort"`.
- The page denies everything by default (`default-src 'none'`,
  `connect-src 'none'`) and allows only its own hashed script and style.
