//! # Seedling
//!
//! A pointer-guided growing plant. A seed waits just under a crust line; as
//! the pointer moves, a shoot grows after it in graceful curves, setting
//! leaves and the odd flower along the way. Rest the pointer and the tip
//! blooms. Slide back along the fresh stem and it shrinks after you. Take the
//! pointer away (or back to the seed) and it withers home to wait.
//!
//! The engine is `no_std`, allocation-free, dependency-free and contains no
//! `unsafe` code. It does not draw anything itself: each frame you call
//! [`Plant::step`] with the elapsed time and pointer position, then
//! [`Plant::draw`] with your own [`Renderer`], which receives plain shapes
//! (strokes, leaves, flowers and the seed) to paint however you like: a
//! browser canvas, a game engine, an SVG file, a terminal.
//!
//! ```
//! use seedling::{Flower, Leaf, Plant, Renderer, Seed, StrokeKind, StrokePoint};
//!
//! /// A renderer that just counts what it is asked to draw.
//! #[derive(Default)]
//! struct Counter { strokes: usize, leaves: usize, flowers: usize }
//!
//! impl Renderer for Counter {
//!     fn seed(&mut self, _: Seed) {}
//!     fn stroke_begin(&mut self, _: StrokeKind, _points: usize) { self.strokes += 1; }
//!     fn stroke_point(&mut self, _: StrokePoint) {}
//!     fn leaf(&mut self, _: Leaf) { self.leaves += 1; }
//!     fn flower(&mut self, _: Flower) { self.flowers += 1; }
//! }
//!
//! let mut plant = Plant::new(800.0, 600.0, 42);
//! for frame in 0..600 {
//!     // Ten seconds at 60 fps, with the pointer drifting across the window.
//!     let x = 400.0 + 250.0 * (frame as f32 * 0.01).sin();
//!     plant.step(1.0 / 60.0, Some((x, 150.0)));
//! }
//! let mut counter = Counter::default();
//! plant.draw(&mut counter);
//! assert!(counter.strokes > 1 && counter.leaves + counter.flowers > 0);
//! ```
//!
//! ## Memory
//!
//! A [`Plant`] keeps its whole state in fixed-size arrays (about 70 KB), so it
//! never allocates. Keep it in a `static`, a `Box` or a long-lived struct
//! rather than moving it around on a small stack.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// Short names (x, y, w, h, a, s…) are the conventional notation in geometry.
#![allow(clippy::many_single_char_names)]
// Determinism: fused multiply-add rounds differently from separate * and +,
// and its speed and availability vary by platform. Seedling promises the same
// plant from the same seed everywhere, so it deliberately does not use
// `mul_add` (the lint only fires in test builds, where std is linked).
#![cfg_attr(test, allow(clippy::suboptimal_flops))]

mod math;

use math::{clamp, dist, fl, norm, rotate, sanitize, sin, smooth, sqrt};

// ---------------------------------------------------------------- capacity --

/// Most stem samples a plant stores (one every 6 px, so about 24,000 px of stem).
pub const MAX_STEM_POINTS: usize = 4000;
/// Most side twigs (each ending in a leaf or flower) a plant keeps at once.
pub const MAX_TWIGS: usize = 600;
/// Most links a plant hands out (see [`Config::links`]).
pub const MAX_LINKS: u8 = 64;

const SEG: f32 = 6.0; // distance between stored stem samples, px
const STEER_GAIN: f32 = 3.4;
const RETRACE_GRAB: f32 = 16.0; // px: pointer this close to recent stem = retrace
const SEED_ZONE: f32 = 36.0; // px: pointer this close to the seed = bury it

// Input limits. Anything outside is clamped, so no arithmetic below can
// overflow to Infinity (e.g. squaring 3.4e38) and turn into NaN.
const COORD_LIMIT: f32 = 100_000.0; // px, pointer coordinates
const SIZE_MIN: f32 = 1.0; // px, viewport
const SIZE_MAX: f32 = 100_000.0;
const DT_MAX: f32 = 0.05; // s; longer frames are treated as 50 ms
const CLOCK_WRAP: f32 = 10_000.0; // s; keeps the animation clock precise forever

// ------------------------------------------------------------------ config --

/// Settings a developer can adjust.
///
/// Start from [`Config::default`] and change the fields you care about;
/// out-of-range values are clamped when applied with [`Plant::set_config`],
/// so no setting can break the simulation.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Config {
    /// Fastest the growing tip may turn, in radians per second. Lower values
    /// give wider, more graceful arcs. Default 2.7; clamped to 0.3..=8.
    pub max_turn: f32,
    /// Slowest growth speed, px/s (used when the tip is close to the
    /// pointer). Default 70; clamped to 10..=2000.
    pub grow_min: f32,
    /// Fastest growth speed, px/s. Default 250; clamped to `grow_min`..=4000.
    pub grow_max: f32,
    /// How strongly the stem meanders on its own. Default 14; 0 = no wander;
    /// clamped to 0..=100.
    pub wander_kick: f32,
    /// How quickly the meander relaxes back to straight. Default 1.8;
    /// clamped to 0.1..=20.
    pub wander_damp: f32,
    /// How visible the older, "remembered" stem stays, from 0 (invisible)
    /// to 1 (fully drawn). Default 0.13.
    pub fog: f32,
    /// Length of stem behind the tip that stays fully vivid, px. Default 260;
    /// clamped to 10..=100000.
    pub fog_reach: f32,
    /// Share of side twigs that end in a flower instead of a leaf, 0..=1.
    /// Default 0.2.
    pub flower_chance: f32,
    /// How many links to hand out to leaves and twig flowers, in growth
    /// order: the first to grow gets link 0, the next link 1, and so on.
    /// When a linked branch withers its link goes to the next one that grows,
    /// so every link stays reachable. See [`Leaf::link`]. Default 0;
    /// clamped to 0..=[`MAX_LINKS`].
    pub links: u8,
    /// Keep the grown plant when the pointer leaves, instead of withering
    /// home. Returning the pointer to the seed still buries it. Default
    /// `false` (the classic behaviour); pages with links usually want `true`
    /// so visitors can come back and click.
    pub persist: bool,
}

impl Config {
    /// The default settings, usable in `const` contexts.
    pub const DEFAULT: Self = Self {
        max_turn: 2.7,
        grow_min: 70.0,
        grow_max: 250.0,
        wander_kick: 14.0,
        wander_damp: 1.8,
        fog: 0.13,
        fog_reach: 260.0,
        flower_chance: 0.2,
        links: 0,
        persist: false,
    };

    /// All-zero placeholder used inside a blank plant (never applied).
    const ZERO: Self = Self {
        max_turn: 0.0,
        grow_min: 0.0,
        grow_max: 0.0,
        wander_kick: 0.0,
        wander_damp: 0.0,
        fog: 0.0,
        fog_reach: 0.0,
        flower_chance: 0.0,
        links: 0,
        persist: false,
    };

    /// This configuration with every field made finite and within range.
    #[must_use]
    pub const fn sanitized(self) -> Self {
        let d = Self::DEFAULT;
        let grow_min = sanitize(self.grow_min, d.grow_min, 10.0, 2000.0);
        Self {
            max_turn: sanitize(self.max_turn, d.max_turn, 0.3, 8.0),
            grow_min,
            grow_max: sanitize(self.grow_max, d.grow_max, grow_min, 4000.0),
            wander_kick: sanitize(self.wander_kick, d.wander_kick, 0.0, 100.0),
            wander_damp: sanitize(self.wander_damp, d.wander_damp, 0.1, 20.0),
            fog: sanitize(self.fog, d.fog, 0.0, 1.0),
            fog_reach: sanitize(self.fog_reach, d.fog_reach, 10.0, 100_000.0),
            flower_chance: sanitize(self.flower_chance, d.flower_chance, 0.0, 1.0),
            links: if self.links > MAX_LINKS { MAX_LINKS } else { self.links },
            persist: self.persist,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::DEFAULT
    }
}

// ------------------------------------------------------------------ output --

/// What a [`Plant`] hands to your renderer each frame, in painting order.
///
/// Coordinates are in the same pixel space as the viewport given to
/// [`Plant::new`] (x to the right, y downward). `alpha` values run from 0
/// (fully faded into the background) to 1 (fully vivid).
pub trait Renderer {
    /// The seed, drawn first. `pulse` cycles 0..=1; `prominence` is 1 while
    /// no plant is showing and lower once it has sprouted.
    fn seed(&mut self, seed: Seed);
    /// Start a stroke of `points` points; exactly that many
    /// [`Renderer::stroke_point`] calls follow.
    fn stroke_begin(&mut self, kind: StrokeKind, points: usize);
    /// One point of the current stroke. Width and alpha vary along a stroke.
    fn stroke_point(&mut self, point: StrokePoint);
    /// A leaf whose base is at (`x`, `y`), pointing along (`dir_x`, `dir_y`).
    fn leaf(&mut self, leaf: Leaf);
    /// A flower or bud. `open` runs from 0 (closed bud) to 1 (fully open).
    fn flower(&mut self, flower: Flower);
}

/// Which part of the plant a stroke belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StrokeKind {
    /// The main stem, from the seed to the growing tip.
    Stem,
    /// A side twig carrying a leaf or flower.
    Twig,
}

/// The seed beneath the crust.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Seed {
    /// Position, px.
    pub x: f32,
    /// Position, px.
    pub y: f32,
    /// Breathing animation phase, 0..=1.
    pub pulse: f32,
    /// 1 while waiting, lower once the plant has sprouted.
    pub prominence: f32,
}

/// One point along a stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct StrokePoint {
    /// Position, px.
    pub x: f32,
    /// Position, px.
    pub y: f32,
    /// Line width at this point, px.
    pub width: f32,
    /// Visibility at this point, 0..=1.
    pub alpha: f32,
}

/// A leaf at the end of a twig.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Leaf {
    /// Base of the leaf, px.
    pub x: f32,
    /// Base of the leaf, px.
    pub y: f32,
    /// Unit direction the leaf points in.
    pub dir_x: f32,
    /// Unit direction the leaf points in.
    pub dir_y: f32,
    /// Length scale, px.
    pub size: f32,
    /// Visibility, 0..=1.
    pub alpha: f32,
    /// Which link this leaf carries (an index into the developer's list of
    /// [`Config::links`] links), or `None` for a plain leaf.
    pub link: Option<u8>,
}

/// A flower or bud: at the growing tip, or at the end of a twig.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Flower {
    /// Centre, px.
    pub x: f32,
    /// Centre, px.
    pub y: f32,
    /// Unit direction the flower faces.
    pub dir_x: f32,
    /// Unit direction the flower faces.
    pub dir_y: f32,
    /// Size, px.
    pub size: f32,
    /// 0 = closed bud, 1 = fully open.
    pub open: f32,
    /// Visibility, 0..=1.
    pub alpha: f32,
    /// Which link this flower carries (an index into the developer's list of
    /// [`Config::links`] links), or `None`. The bud at the growing tip never
    /// carries a link, because it moves.
    pub link: Option<u8>,
}

// ------------------------------------------------------------------- state --

/// What is driving the plant this frame.
#[derive(Clone, Copy, PartialEq)]
enum Drive {
    /// The pointer is at (x, y): grow toward it, or retrace.
    At(f32, f32),
    /// The pointer has gone: wither home.
    Away,
    /// Hold still (pointer over a link, or away with `persist`).
    Rest,
}

/// A side shoot that sprouts from the stem and ends in a leaf or small flower.
#[derive(Clone, Copy)]
struct Twig {
    anchor: usize, // index of the stem sample it grows from
    side: f32,     // -1 left, +1 right of the stem's direction
    spread: f32,   // initial angle away from the stem, rad
    curl: f32,     // bend per px of length, rad/px (arcs back toward the light)
    len: f32,      // full-grown length, px
    g: f32,        // growth 0..1
    flower: bool,
    size: f32,
    link: u8, // 0 = none, k + 1 = link k
}

const TWIG0: Twig = Twig {
    anchor: 0,
    side: 0.0,
    spread: 0.0,
    curl: 0.0,
    len: 0.0,
    g: 0.0,
    flower: false,
    size: 0.0,
    link: 0,
};

/// A growing plant: its whole state, in fixed-size arrays.
pub struct Plant {
    // Settings, valid only when `cfg_set` is true (else the defaults apply).
    // Kept as an all-zero placeholder plus a flag, rather than an Option,
    // so a blank plant is all-zero bytes and a `static` of it costs nothing
    // in the binary (.bss, not .data). An Option<Config> would store "None"
    // as the impossible value 2 in the `persist` bool: one non-zero byte.
    cfg: Config,
    cfg_set: bool,
    w: f32,
    ground: f32,
    sx: f32,
    sy: f32,
    px: [f32; MAX_STEM_POINTS],
    py: [f32; MAX_STEM_POINTS],
    n: usize, // stored samples; px[0],py[0] is the seed. 0 = no viewport yet
    tipx: f32,
    tipy: f32, // growing tip (runs up to SEG px ahead of the last sample)
    hx: f32,
    hy: f32, // heading, unit vector
    wander: f32,
    retract_to: i32,    // -1 = growing; k ≥ 0 = shrinking back to sample k
    retreat_speed: f32, // px/s, fixed when a full retreat begins
    since_twig: f32,
    next_twig: f32,
    last_side: f32,
    twigs: [Twig; MAX_TWIGS],
    nt: usize,
    links_used: u64, // bit k set = link k is on a live twig
    bloom: f32,
    idle: f32,
    t: f32,
    rng: u32,
}

impl Default for Plant {
    fn default() -> Self {
        Self::blank()
    }
}

impl Plant {
    /// A plant with no viewport yet. [`Plant::step`] and [`Plant::draw`] do
    /// nothing until [`Plant::init`] is called. Being `const`, this can
    /// initialise a `static`.
    #[must_use]
    // The arrays are the plant's fixed-size storage. In a `static` this is
    // evaluated at compile time (no stack use); `Plant::new` documents keeping
    // a plant in a static, a Box or a long-lived struct.
    #[allow(clippy::large_stack_arrays)]
    pub const fn blank() -> Self {
        Self {
            cfg: Config::ZERO,
            cfg_set: false,
            w: 0.0,
            ground: 0.0,
            sx: 0.0,
            sy: 0.0,
            px: [0.0; MAX_STEM_POINTS],
            py: [0.0; MAX_STEM_POINTS],
            n: 0,
            tipx: 0.0,
            tipy: 0.0,
            hx: 0.0,
            hy: 0.0,
            wander: 0.0,
            retract_to: 0,
            retreat_speed: 0.0,
            since_twig: 0.0,
            next_twig: 0.0,
            last_side: 0.0,
            twigs: [TWIG0; MAX_TWIGS],
            nt: 0,
            links_used: 0,
            bloom: 0.0,
            idle: 0.0,
            t: 0.0,
            rng: 0,
        }
    }

    /// A fresh seed in a `width` × `height` px viewport. `seed` makes each
    /// plant different; the same seed and inputs always grow the same plant.
    #[must_use]
    pub fn new(width: f32, height: f32, seed: u32) -> Self {
        let mut plant = Self::blank();
        plant.init(width, height, seed);
        plant
    }

    /// Start over with a fresh seed in a `width` × `height` px viewport.
    /// Sizes that are non-finite or out of range are clamped to sane values.
    /// The current [`Config`] is kept.
    pub fn init(&mut self, width: f32, height: f32, seed: u32) {
        let w = sanitize(width, 800.0, SIZE_MIN, SIZE_MAX);
        let h = sanitize(height, 600.0, SIZE_MIN, SIZE_MAX);
        self.w = w;
        self.ground = if h * 0.8 < h - 60.0 { h * 0.8 } else { h - 60.0 };
        if self.ground < h * 0.5 {
            self.ground = h * 0.5; // tiny viewports: keep the crust on screen
        }
        self.sx = w * 0.5;
        self.sy = self.ground + 16.0;
        self.rng = seed | 1; // xorshift must never be seeded with 0
        self.t = 0.0;
        self.reset();
        self.retract_to = 0;
    }

    /// Apply new settings (clamped into range). Takes effect immediately.
    pub const fn set_config(&mut self, config: Config) {
        self.cfg = config.sanitized();
        self.cfg_set = true;
    }

    /// The settings in effect (after clamping).
    #[must_use]
    pub const fn config(&self) -> Config {
        if self.cfg_set { self.cfg } else { Config::DEFAULT }
    }

    /// The y coordinate of the crust line, px.
    #[must_use]
    pub const fn ground_y(&self) -> f32 {
        self.ground
    }

    /// Where the seed sits, px.
    #[must_use]
    pub const fn seed_position(&self) -> (f32, f32) {
        (self.sx, self.sy)
    }

    /// True once a shoot has sprouted (false while only the seed shows).
    #[must_use]
    pub fn has_plant(&self) -> bool {
        self.n > 1 || dist(self.tipx, self.tipy, self.sx, self.sy) > 0.5
    }

    /// Advance the simulation by `dt` seconds (clamped to 0..=0.05).
    ///
    /// `pointer` is the pointer position in viewport pixels, or `None` when
    /// the pointer has left (which makes the plant wither home, unless
    /// [`Config::persist`] is set). Non-finite values are treated as `None`.
    pub fn step(&mut self, dt: f32, pointer: Option<(f32, f32)>) {
        match pointer {
            Some((x, y)) if x.is_finite() && y.is_finite() => self.advance(
                dt,
                Drive::At(clamp(x, -COORD_LIMIT, COORD_LIMIT), clamp(y, -COORD_LIMIT, COORD_LIMIT)),
            ),
            _ => self.advance(dt, Drive::Away),
        }
    }

    /// Advance the animation by `dt` seconds without growing or withering:
    /// the plant holds still, finishes unfolding its leaves and blooms at the
    /// tip. Call this instead of [`Plant::step`] while the pointer is over a
    /// link, so reaching for a leaf doesn't make the branch chase or prune
    /// itself.
    pub fn rest(&mut self, dt: f32) {
        self.advance(dt, Drive::Rest);
    }

    /// Hand this frame's shapes to `renderer`, in painting order: seed, twig
    /// strokes, the stem, leaves and twig flowers, then the bud or flower at
    /// the tip. Does nothing before [`Plant::init`].
    pub fn draw<R: Renderer + ?Sized>(&self, renderer: &mut R) {
        if self.n == 0 {
            return;
        }
        renderer.seed(Seed {
            x: self.sx,
            y: self.sy,
            pulse: 0.5 + 0.5 * sin(self.t * 2.6),
            prominence: if self.has_plant() { 0.35 } else { 1.0 },
        });
        if !self.has_plant() {
            return;
        }

        // Twig strokes first, so the main stem overlaps their bases.
        let mut ends = [[0.0_f32; 4]; MAX_TWIGS];
        for (k, end) in ends.iter_mut().enumerate().take(self.nt) {
            *end = self.draw_twig(k, renderer);
        }

        renderer.stroke_begin(StrokeKind::Stem, self.n + 1);
        for i in 0..self.n {
            let (x, y) = (self.px[i], self.py[i]);
            renderer.stroke_point(StrokePoint {
                x: x + self.sway(x, y),
                y,
                width: self.width_at(i),
                alpha: self.fog_at(i),
            });
        }
        let (tx, ty) = (self.tipx, self.tipy);
        let tsx = tx + self.sway(tx, ty);
        renderer.stroke_point(StrokePoint { x: tsx, y: ty, width: self.width_at(self.n), alpha: 1.0 });

        // Leaves and twig flowers on top.
        for (tw, e) in self.twigs.iter().zip(ends.iter()).take(self.nt) {
            let alpha = self.fog_at(tw.anchor);
            if tw.flower {
                renderer.flower(Flower {
                    x: e[0],
                    y: e[1],
                    dir_x: e[2],
                    dir_y: e[3],
                    size: tw.size * 7.0 * smooth(tw.g / 0.6),
                    open: smooth((tw.g - 0.5) / 0.5),
                    alpha,
                    link: self.link_of(tw),
                });
            } else {
                let size = tw.size * 13.0 * smooth((tw.g - 0.25) / 0.75);
                if size > 0.3 {
                    renderer.leaf(Leaf {
                        x: e[0],
                        y: e[1],
                        dir_x: e[2],
                        dir_y: e[3],
                        size,
                        alpha,
                        link: self.link_of(tw),
                    });
                }
            }
        }

        // The tip: a closed bud while growing, a flower while resting.
        if self.n > 2 && self.tipy < self.ground {
            renderer.flower(Flower {
                x: tsx,
                y: ty,
                dir_x: self.hx,
                dir_y: self.hy,
                size: 5.5 + 11.0 * self.bloom,
                open: self.bloom,
                alpha: 1.0,
                link: None,
            });
        }
    }

    // ------------------------------------------------------------ internals

    fn advance(&mut self, dt: f32, drive: Drive) {
        if self.n == 0 {
            return; // no viewport yet
        }
        let dt = sanitize(dt, 0.0, 0.0, DT_MAX);
        self.t += dt;
        if self.t > CLOCK_WRAP {
            self.t -= CLOCK_WRAP;
        }
        let drive = match drive {
            Drive::Away if self.config().persist => Drive::Rest,
            other => other,
        };
        self.update(dt, drive);
        self.heal();
    }

    /// Lowest free link index below `Config::links`, marked used; 0 if none.
    const fn take_link(&mut self) -> u8 {
        let mut i = 0;
        while i < self.config().links {
            let bit = 1u64 << i;
            if self.links_used & bit == 0 {
                self.links_used |= bit;
                return i + 1;
            }
            i += 1;
        }
        0
    }

    /// The public link index of `tw`, if it has one still within range.
    const fn link_of(&self, tw: &Twig) -> Option<u8> {
        if tw.link > 0 && tw.link - 1 < self.config().links { Some(tw.link - 1) } else { None }
    }

    // xorshift32: small, fast, and good enough to make each plant different.
    const fn rnd(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        // 24 random bits → [0, 1). Values < 2^24 convert to f32 exactly.
        #[allow(clippy::cast_precision_loss)]
        let r = (x >> 8) as f32 / 16_777_216.0;
        r
    }

    const fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.rnd()
    }

    const fn reset(&mut self) {
        self.n = 1;
        self.px[0] = self.sx;
        self.py[0] = self.sy;
        self.tipx = self.sx;
        self.tipy = self.sy;
        self.hx = 0.0;
        self.hy = -1.0;
        self.wander = 0.0;
        self.retract_to = -1;
        self.nt = 0;
        self.links_used = 0;
        self.bloom = 0.0;
        self.idle = 0.0;
        self.since_twig = 0.0;
        self.next_twig = 30.0;
        self.last_side = 1.0;
    }

    /// Defence in depth: if anything ever went non-finite, start over rather
    /// than drawing garbage forever. (Input sanitizing should make this
    /// unreachable; the fuzz harness and tests check that it is.)
    const fn heal(&mut self) {
        let ok = self.tipx.is_finite()
            && self.tipy.is_finite()
            && self.hx.is_finite()
            && self.hy.is_finite()
            && self.wander.is_finite()
            && self.bloom.is_finite()
            && self.idle.is_finite();
        if !ok {
            self.reset();
            self.retract_to = 0;
        }
    }

    fn update(&mut self, dt: f32, drive: Drive) {
        let (active, resting, mx, my) = match drive {
            Drive::At(x, y) => (true, false, x, y),
            Drive::Rest => (false, true, 0.0, 0.0),
            Drive::Away => (false, false, 0.0, 0.0),
        };
        let near_seed = active && dist(mx, my, self.sx, self.ground) < SEED_ZONE;

        if resting {
            // Hold still: no growth and no retreat.
        } else if !active || near_seed {
            // Pointer left the window, or came home to the seed: wither back.
            // Speed is fixed now so the whole stem goes home in ~1.2 s,
            // rather than slowing down as it shortens.
            if self.retract_to != 0 {
                let total = fl(self.n) * SEG;
                self.retreat_speed = if total / 1.2 > 380.0 { total / 1.2 } else { 380.0 };
            }
            self.retract_to = 0;
        } else {
            if self.retract_to == 0 {
                self.retract_to = -1; // pointer is back: resume growing
            }
            self.detect_retrace(mx, my);
        }

        if resting {
            self.idle += dt;
        } else if self.retract_to >= 0 {
            self.retract(dt);
            self.idle = 0.0;
        } else {
            // The pointer can't lead the plant into the soil.
            self.grow(dt, mx, if my > self.ground - 14.0 { self.ground - 14.0 } else { my });
        }

        // Twigs: grow, or wither as the retreating tip approaches them.
        let retracting = !resting && self.retract_to >= 0;
        let mut k = 0;
        while k < self.nt {
            let mut tw = self.twigs[k];
            let withering = retracting && tw.anchor + 6 > self.n;
            if withering {
                tw.g -= dt * 5.0;
            } else if tw.g < 1.0 {
                tw.g = if tw.g + dt * 1.4 > 1.0 { 1.0 } else { tw.g + dt * 1.4 };
            }
            if tw.anchor >= self.n || (withering && tw.g <= 0.0) {
                if tw.link > 0 {
                    self.links_used &= !(1u64 << (tw.link - 1)); // free its link
                }
                self.nt -= 1;
                self.twigs[k] = self.twigs[self.nt];
                continue;
            }
            self.twigs[k] = tw;
            k += 1;
        }

        // The tip blooms when the pointer rests; folds back up when it moves.
        if !retracting && (active || resting) && self.idle > 0.45 && self.n > 8 {
            self.bloom = clamp(self.bloom + dt * 1.1, 0.0, 1.0);
        } else {
            self.bloom = clamp(self.bloom - dt * 2.8, 0.0, 1.0);
        }
    }

    /// Pointer sliding back over the freshest part of the stem = retrace.
    /// Only the last ~45 samples count, so sweeping across old stem far from
    /// the tip doesn't prune the plant.
    fn detect_retrace(&mut self, mx: f32, my: f32) {
        if self.n < 8 || dist(mx, my, self.tipx, self.tipy) < 22.0 {
            return;
        }
        let lo = if self.n > 46 { self.n - 46 } else { 1 };
        let hi = self.n - 6;
        let mut best = -1i32;
        let mut best_d = RETRACE_GRAB;
        let mut i = lo;
        while i < hi {
            let d = dist(mx, my, self.px[i], self.py[i]);
            if d < best_d {
                best_d = d;
                best = i32::try_from(i).unwrap_or(i32::MAX);
            }
            i += 1;
        }
        if best >= 0 && (self.retract_to < 0 || best < self.retract_to) {
            self.retract_to = best;
        }
    }

    fn retract(&mut self, dt: f32) {
        let target = usize::try_from(self.retract_to).unwrap_or(0);
        let speed = if target == 0 {
            if self.retreat_speed > 380.0 { self.retreat_speed } else { 380.0 }
        } else {
            360.0
        };
        let mut rem = speed * dt;
        loop {
            let lx = self.px[self.n - 1];
            let ly = self.py[self.n - 1];
            let d = dist(self.tipx, self.tipy, lx, ly);
            if d > rem {
                let k = rem / d;
                self.tipx += (lx - self.tipx) * k;
                self.tipy += (ly - self.tipy) * k;
                return;
            }
            self.tipx = lx;
            self.tipy = ly;
            rem -= d;
            if self.n - 1 > target {
                self.n -= 1;
            } else {
                // Arrived. A full retreat stays latched until the pointer
                // returns; a partial one hands control back to growth.
                if target == 0 {
                    self.reset();
                    self.retract_to = 0;
                } else {
                    self.retract_to = -1;
                    let i = self.n - 1;
                    let (hx, hy) = norm(
                        self.px[i] - self.px[i - 1],
                        self.py[i] - self.py[i - 1],
                        0.0,
                        -1.0,
                    );
                    self.hx = hx;
                    self.hy = hy;
                    self.wander = 0.0;
                }
                return;
            }
        }
    }

    fn grow(&mut self, dt: f32, tx: f32, ty: f32) {
        let ddx = tx - self.tipx;
        let ddy = ty - self.tipy;
        let d = sqrt(ddx * ddx + ddy * ddy);
        if d < 9.0 {
            self.idle += dt; // arrived: rest (and maybe bloom)
            return;
        }
        self.idle = 0.0;
        let ux = ddx / d;
        let uy = ddy / d;

        // 1. Steering: turn toward the pointer, but never faster than max_turn.
        //    cross > 0 means the pointer is clockwise of our heading.
        let cross = self.hx * uy - self.hy * ux;
        let dot = self.hx * ux + self.hy * uy;
        let mut turn = if dot > 0.0 {
            cross * STEER_GAIN
        } else if cross >= 0.0 {
            STEER_GAIN // pointer is behind us: commit to a wide U-turn
        } else {
            -STEER_GAIN
        };
        turn = clamp(turn, -self.config().max_turn, self.config().max_turn);

        // 2. Wander: a damped random walk on the turn rate. This is the
        //    "randomness" — smooth, never jittery, different every time.
        self.wander += (self.rnd() - 0.5) * self.config().wander_kick * dt;
        self.wander -= self.wander * self.config().wander_damp * dt;
        turn += self.wander;

        // 3. Up-seeking (gravitropism): underground, and just above the crust,
        //    the shoot pulls toward vertical. cross(heading, up) = -hx.
        let below = self.tipy > self.ground;
        if below {
            turn = -self.hx * 6.0;
        } else if self.tipy > self.ground - 18.0 && self.hy > -0.2 {
            turn += -self.hx * 4.0;
        }

        let (hx, hy) = rotate(self.hx, self.hy, turn * dt);
        let (hx, hy) = norm(hx, hy, 0.0, -1.0);
        self.hx = hx;
        self.hy = hy;

        let speed = clamp(d * 2.2, self.config().grow_min, self.config().grow_max);
        self.tipx = clamp(self.tipx + hx * speed * dt, 4.0, self.w - 4.0);
        self.tipy += hy * speed * dt;
        if !below && self.tipy > self.ground - 3.0 {
            self.tipy = self.ground - 3.0; // once out of the soil, stay out
        }
        if self.tipy < 4.0 {
            self.tipy = 4.0;
        }

        // Record a sample every SEG px; sprout twigs along the way.
        let li = self.n - 1;
        if dist(self.tipx, self.tipy, self.px[li], self.py[li]) >= SEG && self.n < MAX_STEM_POINTS {
            self.px[self.n] = self.tipx;
            self.py[self.n] = self.tipy;
            self.n += 1;
            self.since_twig += SEG;
            if self.since_twig >= self.next_twig && self.n > 5 && self.tipy < self.ground - 10.0 {
                self.spawn_twigs();
            }
        }
    }

    fn spawn_twigs(&mut self) {
        self.since_twig = 0.0;
        self.next_twig = self.range(24.0, 56.0);
        // Mostly alternate sides, like real phyllotaxis — with exceptions.
        let side = if self.rnd() < 0.22 { self.last_side } else { -self.last_side };
        self.last_side = side;
        self.add_twig(side);
        if self.rnd() < 0.14 {
            self.add_twig(-side); // occasional opposite pair
        }
    }

    fn add_twig(&mut self, side: f32) {
        if self.nt >= MAX_TWIGS {
            return;
        }
        let tw = Twig {
            anchor: self.n - 1,
            side,
            spread: self.range(0.55, 1.05),
            curl: -side * self.range(0.008, 0.03),
            len: self.range(14.0, 42.0),
            g: 0.0,
            flower: self.rnd() < self.config().flower_chance,
            size: self.range(0.8, 1.25),
            link: 0,
        };
        let tw = Twig { link: self.take_link(), ..tw };
        self.twigs[self.nt] = tw;
        self.nt += 1;
    }

    /// Gentle breathing sway; grows with height above the crust.
    fn sway(&self, x: f32, y: f32) -> f32 {
        let lift = clamp((self.ground - y) / 280.0, 0.0, 1.0);
        sin(self.t * 1.15 + y * 0.012 + x * 0.003) * 2.0 * lift
    }

    /// Fog: vivid near the tip, fading to a faint outline behind it.
    fn fog_at(&self, i: usize) -> f32 {
        let li = self.n - 1;
        let tail = dist(self.tipx, self.tipy, self.px[li], self.py[li]);
        let d = fl(li - i) * SEG + tail;
        self.config().fog + (1.0 - self.config().fog) * smooth(1.0 - d / self.config().fog_reach)
    }

    /// Taper: thick at the seed, thin at the tip; the whole stem thickens
    /// as it gets longer.
    fn width_at(&self, i: usize) -> f32 {
        let m = fl(self.n) + 1.0;
        let r = (m - fl(i)) / (m + 40.0);
        0.9 + 4.8 * sqrt(clamp(r, 0.0, 1.0))
    }

    /// Draw twig `k`'s stroke and return its tip `[x, y, dir_x, dir_y]`.
    fn draw_twig<R: Renderer + ?Sized>(&self, k: usize, renderer: &mut R) -> [f32; 4] {
        let tw = self.twigs[k];
        let a = tw.anchor;
        if a >= self.n || tw.g <= 0.02 {
            let i = a.min(self.n - 1);
            return [self.px[i], self.py[i], 0.0, -1.0];
        }
        // Stem tangent at the anchor.
        let i0 = if a > 0 { a - 1 } else { 0 };
        let i1 = if a + 1 < self.n { a + 1 } else { self.n - 1 };
        let (tx, ty) = norm(self.px[i1] - self.px[i0], self.py[i1] - self.py[i0], self.hx, self.hy);
        let (mut hx, mut hy) = rotate(tx, ty, tw.side * tw.spread);
        let length = tw.len * smooth(tw.g);
        // Clamped to 1..=11 first, so the cast can neither wrap nor go negative.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = clamp((length / 4.0) + 1.0, 1.0, 11.0) as usize;
        let seg = length / fl(steps);
        let alpha = self.fog_at(a);
        let w0 = self.width_at(a) * 0.5;

        let (mut x, mut y) = (self.px[a], self.py[a]);
        renderer.stroke_begin(StrokeKind::Twig, steps + 1);
        renderer.stroke_point(StrokePoint { x: x + self.sway(x, y), y, width: w0, alpha });
        for s in 1..=steps {
            let (rx, ry) = rotate(hx, hy, tw.curl * seg);
            hx = rx;
            hy = ry;
            x += hx * seg;
            y += hy * seg;
            renderer.stroke_point(StrokePoint {
                x: x + self.sway(x, y),
                y,
                width: w0 + (0.6 - w0) * (fl(s) / fl(steps)),
                alpha,
            });
        }
        [x + self.sway(x, y), y, hx, hy]
    }
}

#[cfg(test)]
mod tests;
