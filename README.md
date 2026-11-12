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
