# seedling

A pointer-guided growing plant, as a small Rust library.

A seed waits just under a crust line. As the pointer moves, a shoot grows
after it in graceful curves, setting leaves and the odd flower along the way.
Rest the pointer and the tip blooms; slide back along the fresh stem and it
shrinks after you; take the pointer away and it withers home to wait.

- **`no_std`, no allocation, no dependencies.** The whole state lives in
  fixed-size arrays (about 70 KB).
- **No `unsafe` code**, enforced with `#![forbid(unsafe_code)]`.
- **Hardened input handling.** NaN, infinite or absurd values for time,
  pointer, viewport or settings are clamped, never propagated.
- **Deterministic.** The same seed and inputs always grow the same plant, on
  every platform.
- **Bring your own renderer.** The engine hands you shapes (strokes, leaves,
  flowers, the seed); you paint them on a canvas, in a game engine, to SVG...

## Example

```rust
use seedling::{Config, Flower, Leaf, Plant, Renderer, Seed, StrokeKind, StrokePoint};

struct MyCanvas; // your drawing surface

impl Renderer for MyCanvas {
    fn seed(&mut self, s: Seed) { /* draw a small glowing seed at (s.x, s.y) */ }
    fn stroke_begin(&mut self, kind: StrokeKind, points: usize) { /* start a path */ }
    fn stroke_point(&mut self, p: StrokePoint) { /* add (p.x, p.y), width p.width, opacity p.alpha */ }
    fn leaf(&mut self, l: Leaf) { /* leaf at (l.x, l.y) pointing along (l.dir_x, l.dir_y) */ }
    fn flower(&mut self, f: Flower) { /* flower, f.open from 0 (bud) to 1 (open) */ }
}

let mut plant = Plant::new(800.0, 600.0, 42); // viewport size, random seed

let mut settings = Config::default();
settings.max_turn = 2.0;     // wider, lazier curves
settings.flower_chance = 0.5; // more flowers
plant.set_config(settings);

// Each frame:
let pointer = Some((420.0, 180.0)); // or None when the pointer leaves
plant.step(1.0 / 60.0, pointer);
plant.draw(&mut MyCanvas);
```

## Growth styles

- `Style::Vine` (always available): the curving vine described above.
- `Style::CircuitBirch` (Cargo feature `circuit`): a circuit-board tree. A
  straight trunk rises toward the pointer; branches route toward it like
  printed-circuit traces (straight runs, 45° turns) and end in birch leaves.
  Branch length, trunk height and branch spacing are `Config` settings.

```toml
seedling = { version = "0.1", features = ["circuit"] }
```

Leaves and flowers can carry links (`Config::links`), handed out in growth
order; your renderer receives the link index on each `Leaf` and `Flower`.

## Live demo

The repository includes `seedling.html`, a self-contained page that runs this
engine compiled to WebAssembly. Open it in any modern browser; no server or
install needed.

## Security

Every change passes a scored security gate (linting, unit tests, a
1,000,000-frame fuzz of the WebAssembly build, browser and Content Security
Policy checks) before it can be published. See the repository's `SECURITY.md`
to report a vulnerability privately.

## License

Licensed under either of MIT or Apache-2.0, at your option.
Copyright (c) 2026 Scott Lingley. Built with substantial assistance from
Claude (Anthropic); see `NOTICE` and `AUTHORS.md`.
