//! Seedling — the growth engine.
//!
//! Everything about *how* the plant grows lives here: steering toward the
//! pointer, the curvature limit that keeps branches graceful, the random
//! "wander" that makes each growth unique, side twigs, leaves, flowers,
//! retracing and withering back to the seed.
//!
//! The browser's JavaScript does only two things: tell this module where the
//! pointer is each frame, and paint the list of shapes this module hands back.
//!
//! `#![no_std]` means no operating system and no allocator: all state lives in
//! fixed-size arrays, so the compiled .wasm is tiny and needs nothing from the
//! page except a frame clock.

#![no_std]
// Security posture: every `unsafe` must carry a SAFETY justification.
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
// Short names (x, y, w, h, a, s…) are the conventional notation in geometry.
#![allow(clippy::many_single_char_names)]

use core::arch::wasm32;
use core::f32::consts::{PI, TAU};
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    wasm32::unreachable()
}

// ---------------------------------------------------------------- tunables --

const MAX_PTS: usize = 4000; // stem samples (≈ 24,000 px of stem)
const MAX_TWIGS: usize = 600;
const MAX_OUT: usize = 120_000; // floats in the draw-command buffer
const SEG: f32 = 6.0; // distance between stored stem samples, px

const GROW_MIN: f32 = 70.0; // px/s
const GROW_MAX: f32 = 250.0; // px/s
const MAX_TURN: f32 = 2.7; // rad/s. At ~180 px/s this caps curvature at a
                           // radius of ~65 px: no kinks, only arcs.
const STEER_GAIN: f32 = 3.4;
const WANDER_KICK: f32 = 14.0; // random-walk strength of the heading noise
const WANDER_DAMP: f32 = 1.8; // how fast the noise relaxes back to zero

const FOG: f32 = 0.13; // how visible the "remembered" stem stays (0..1)
const FOG_REACH: f32 = 260.0; // px of stem behind the tip that stay vivid

const RETRACE_GRAB: f32 = 16.0; // px: pointer this close to recent stem = retrace
const SEED_ZONE: f32 = 36.0; // px: pointer this close to the seed = bury it

// Input limits. Anything outside is clamped, so no arithmetic below can
// overflow to Infinity (e.g. squaring 3.4e38) and turn into NaN.
const COORD_LIMIT: f32 = 100_000.0; // px, pointer coordinates
const SIZE_MIN: f32 = 1.0; // px, viewport
const SIZE_MAX: f32 = 100_000.0;
const CLOCK_WRAP: f32 = 10_000.0; // s; keeps the animation clock precise forever

// Draw commands written to the output buffer (read by the page).
const CMD_END: f32 = 0.0;
const CMD_STROKE: f32 = 1.0; // kind, count, then count × (x, y, width, alpha)
const CMD_LEAF: f32 = 2.0; // x, y, dirx, diry, size, alpha
const CMD_FLOWER: f32 = 3.0; // x, y, dirx, diry, size, open, alpha
const CMD_SEED: f32 = 4.0; // x, y, pulse, prominence

// -------------------------------------------------------------- tiny math --
// core (no_std) has no sin/cos/sqrt on stable, so they are written out here.

fn sqrt(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    // Halving the float's exponent bits gives a close first guess;
    // three Newton steps (y ← (y + x/y)/2) make it exact to f32 precision.
    let mut y = f32::from_bits((x.to_bits() >> 1) + 0x1fbd_1df5);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    0.5 * (y + x / y)
}

fn sin(x: f32) -> f32 {
    // Reduce to [-π, π], fold to [-π/2, π/2], then a 7th-order Taylor series
    // (max error ≈ 1.6e-4 — far below a pixel at these scales).
    let q = x / TAU;
    // `as i32` saturates for huge inputs (Rust defines float→int casts as
    // saturating, never UB); |turns| < 2^24 here, so the result is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let turns = (if q >= 0.0 { q + 0.5 } else { q - 0.5 }) as i32 as f32;
    let mut x = x - TAU * turns;
    let h = PI * 0.5;
    if x > h {
        x = PI - x;
    } else if x < -h {
        x = -PI - x;
    }
    let x2 = x * x;
    x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0)))
}

#[inline]
fn cos(x: f32) -> f32 {
    sin(x + PI * 0.5)
}

#[inline]
fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

/// Index or count → float. Every count in this engine is bounded by
/// `MAX_OUT` (120,000) < 2^24, so the conversion is exact.
#[inline]
#[allow(clippy::cast_precision_loss)]
const fn fl(n: usize) -> f32 {
    n as f32
}

/// Replace NaN/±Infinity with a default, then clamp into [lo, hi].
#[inline]
fn sanitize(x: f32, default: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() { clamp(x, lo, hi) } else { default }
}

#[inline]
fn smooth(x: f32) -> f32 {
    let u = clamp(x, 0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

#[inline]
fn dist(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    sqrt(dx * dx + dy * dy)
}

#[inline]
fn norm(x: f32, y: f32, fx: f32, fy: f32) -> (f32, f32) {
    let l = sqrt(x * x + y * y);
    if l < 1e-6 { (fx, fy) } else { (x / l, y / l) }
}

#[inline]
fn rotate(x: f32, y: f32, a: f32) -> (f32, f32) {
    // Screen space (y points down): positive angle turns clockwise on screen.
    let (c, s) = (cos(a), sin(a));
    (x * c - y * s, x * s + y * c)
}

// ------------------------------------------------------------------ state --

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
}

// All-zero so the whole world lands in .bss and adds nothing to the .wasm size.
const TWIG0: Twig = Twig {
    anchor: 0,
    side: 0.0,
    spread: 0.0,
    curl: 0.0,
    len: 0.0,
    g: 0.0,
    flower: false,
    size: 0.0,
};

struct World {
    w: f32,
    h: f32,
    ground: f32,
    sx: f32,
    sy: f32,
    px: [f32; MAX_PTS],
    py: [f32; MAX_PTS],
    n: usize, // stored samples; px[0],py[0] is the seed
    tipx: f32,
    tipy: f32, // growing tip (runs up to SEG px ahead of the last sample)
    hx: f32,
    hy: f32, // heading, unit vector
    wander: f32,
    retract_to: i32, // -1 = growing; k ≥ 0 = shrinking back to sample k
    retreat_speed: f32, // px/s, fixed when a full retreat begins
    since_twig: f32,
    next_twig: f32,
    last_side: f32,
    twigs: [Twig; MAX_TWIGS],
    nt: usize,
    tend: [[f32; 4]; MAX_TWIGS], // per-frame twig tip x, y, dirx, diry
    bloom: f32,
    idle: f32,
    t: f32,
    rng: u32,
    out: [f32; MAX_OUT],
    on: usize,
}

static mut WORLD: World = World {
    w: 0.0,
    h: 0.0,
    ground: 0.0,
    sx: 0.0,
    sy: 0.0,
    px: [0.0; MAX_PTS],
    py: [0.0; MAX_PTS],
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
    tend: [[0.0; 4]; MAX_TWIGS],
    bloom: 0.0,
    idle: 0.0,
    t: 0.0,
    rng: 0,
    out: [0.0; MAX_OUT],
    on: 0,
};

/// Access the single simulation state.
///
/// # Safety
/// The caller must not hold any other reference into `WORLD` while the
/// returned one is alive. Each exported function below calls this exactly
/// once and drops the reference before returning; WebAssembly here is
/// single-threaded and the module imports nothing, so no export can
/// re-enter another while it runs.
#[inline]
unsafe fn world() -> &'static mut World {
    // SAFETY: uniqueness is guaranteed by the caller contract above.
    // `addr_of_mut!` takes the address without creating an intermediate
    // reference to the `static mut`.
    unsafe { &mut *core::ptr::addr_of_mut!(WORLD) }
}

impl World {
    // xorshift32: small, fast, and good enough to make each plant different.
    fn rnd(&mut self) -> f32 {
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

    fn range(&mut self, a: f32, b: f32) -> f32 {
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
        self.bloom = 0.0;
        self.idle = 0.0;
        self.since_twig = 0.0;
        self.next_twig = 30.0;
        self.last_side = 1.0;
    }

    /// Defence in depth: if anything ever went non-finite, start over rather
    /// than drawing garbage forever. (Input sanitizing should make this
    /// unreachable; the fuzz harness checks that it is.)
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

    fn has_plant(&self) -> bool {
        self.n > 1 || dist(self.tipx, self.tipy, self.sx, self.sy) > 0.5
    }

    // ---------------------------------------------------------- simulation --

    fn update(&mut self, dt: f32, mx: f32, my: f32, active: bool) {
        let near_seed = dist(mx, my, self.sx, self.ground) < SEED_ZONE;

        if !active || near_seed {
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

        if self.retract_to >= 0 {
            self.retract(dt);
            self.idle = 0.0;
        } else {
            // The pointer can't lead the plant into the soil.
            self.grow(dt, mx, if my > self.ground - 14.0 { self.ground - 14.0 } else { my });
        }

        // Twigs: grow, or wither as the retreating tip approaches them.
        let retracting = self.retract_to >= 0;
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
                self.nt -= 1;
                self.twigs[k] = self.twigs[self.nt];
                continue;
            }
            self.twigs[k] = tw;
            k += 1;
        }

        // The tip blooms when the pointer rests; folds back up when it moves.
        if !retracting && active && self.idle > 0.45 && self.n > 8 {
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

        // 1. Steering: turn toward the pointer, but never faster than MAX_TURN.
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
        turn = clamp(turn, -MAX_TURN, MAX_TURN);

        // 2. Wander: a damped random walk on the turn rate. This is the
        //    "randomness" — smooth, never jittery, different every time.
        self.wander += (self.rnd() - 0.5) * WANDER_KICK * dt;
        self.wander -= self.wander * WANDER_DAMP * dt;
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

        let speed = clamp(d * 2.2, GROW_MIN, GROW_MAX);
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
        if dist(self.tipx, self.tipy, self.px[li], self.py[li]) >= SEG && self.n < MAX_PTS {
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
            flower: self.rnd() < 0.2,
            size: self.range(0.8, 1.25),
        };
        self.twigs[self.nt] = tw;
        self.nt += 1;
    }

    // ------------------------------------------------------------- drawing --

    const fn put(&mut self, v: f32) {
        if self.on < MAX_OUT {
            self.out[self.on] = v;
            self.on += 1;
        }
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
        FOG + (1.0 - FOG) * smooth(1.0 - d / FOG_REACH)
    }

    /// Taper: thick at the seed, thin at the tip; the whole stem thickens
    /// as it gets longer.
    fn width_at(&self, i: usize) -> f32 {
        let m = fl(self.n) + 1.0;
        let r = (m - fl(i)) / (m + 40.0);
        0.9 + 4.8 * sqrt(clamp(r, 0.0, 1.0))
    }

    fn emit(&mut self) {
        self.on = 0;
        let prominence = if self.has_plant() { 0.35 } else { 1.0 };
        let pulse = 0.5 + 0.5 * sin(self.t * 2.6);
        let (sx, sy) = (self.sx, self.sy);
        self.put(CMD_SEED);
        self.put(sx);
        self.put(sy);
        self.put(pulse);
        self.put(prominence);

        if self.has_plant() {
            // Twig stems first, so the main stem overlaps their bases.
            for k in 0..self.nt {
                self.emit_twig(k);
            }

            let m = self.n + 1;
            self.put(CMD_STROKE);
            self.put(0.0);
            self.put(fl(m));
            for i in 0..self.n {
                let (x, y) = (self.px[i], self.py[i]);
                let x = x + self.sway(x, y);
                let w = self.width_at(i);
                let a = self.fog_at(i);
                self.put(x);
                self.put(y);
                self.put(w);
                self.put(a);
            }
            let (tx, ty) = (self.tipx, self.tipy);
            let tsx = tx + self.sway(tx, ty);
            let tw = self.width_at(self.n);
            self.put(tsx);
            self.put(ty);
            self.put(tw);
            self.put(1.0);

            // Leaves and twig flowers on top.
            for k in 0..self.nt {
                let tw = self.twigs[k];
                let e = self.tend[k];
                let a = self.fog_at(tw.anchor);
                if tw.flower {
                    let s = tw.size * 7.0 * smooth(tw.g / 0.6);
                    let open = smooth((tw.g - 0.5) / 0.5);
                    self.put(CMD_FLOWER);
                    for v in [e[0], e[1], e[2], e[3], s, open, a] {
                        self.put(v);
                    }
                } else {
                    let s = tw.size * 13.0 * smooth((tw.g - 0.25) / 0.75);
                    if s > 0.3 {
                        self.put(CMD_LEAF);
                        for v in [e[0], e[1], e[2], e[3], s, a] {
                            self.put(v);
                        }
                    }
                }
            }

            // The tip: a closed bud while growing, a flower while resting.
            if self.n > 2 && self.tipy < self.ground {
                let (hx, hy, b) = (self.hx, self.hy, self.bloom);
                self.put(CMD_FLOWER);
                for v in [tsx, ty, hx, hy, 5.5 + 11.0 * b, b, 1.0] {
                    self.put(v);
                }
            }
        }
        self.put(CMD_END);
    }

    fn emit_twig(&mut self, k: usize) {
        let tw = self.twigs[k];
        let a = tw.anchor;
        if a >= self.n || tw.g <= 0.02 {
            self.tend[k] = [self.px[a.min(self.n - 1)], self.py[a.min(self.n - 1)], 0.0, -1.0];
            return;
        }
        // Stem tangent at the anchor.
        let i0 = if a > 0 { a - 1 } else { 0 };
        let i1 = if a + 1 < self.n { a + 1 } else { self.n - 1 };
        let (tx, ty) = norm(
            self.px[i1] - self.px[i0],
            self.py[i1] - self.py[i0],
            self.hx,
            self.hy,
        );
        let (mut hx, mut hy) = rotate(tx, ty, tw.side * tw.spread);
        let length = tw.len * smooth(tw.g);
        // Clamped to 1..=11 first, so the cast can neither wrap nor go negative.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = clamp((length / 4.0) + 1.0, 1.0, 11.0) as usize;
        let seg = length / fl(steps);
        let alpha = self.fog_at(a);
        let w0 = self.width_at(a) * 0.5;

        let (mut x, mut y) = (self.px[a], self.py[a]);
        self.put(CMD_STROKE);
        self.put(1.0);
        self.put(fl(steps + 1));
        let sx = x + self.sway(x, y);
        self.put(sx);
        self.put(y);
        self.put(w0);
        self.put(alpha);
        for s in 1..=steps {
            let (rx, ry) = rotate(hx, hy, tw.curl * seg);
            hx = rx;
            hy = ry;
            x += hx * seg;
            y += hy * seg;
            let w = w0 + (0.6 - w0) * (fl(s) / fl(steps));
            let sx = x + self.sway(x, y);
            self.put(sx);
            self.put(y);
            self.put(w);
            self.put(alpha);
        }
        let ex = x + self.sway(x, y);
        self.tend[k] = [ex, y, hx, hy];
    }
}

// ---------------------------------------------------- exports to the page --

/// Set up a fresh world. `seed` makes each visit's plant different.
/// Sizes that are non-finite or out of range are clamped to sane values.
#[unsafe(no_mangle)]
pub extern "C" fn init(w: f32, h: f32, seed: u32) {
    // SAFETY: the only reference to WORLD during this call (see `world`).
    let s = unsafe { world() };
    let w = sanitize(w, 800.0, SIZE_MIN, SIZE_MAX);
    let h = sanitize(h, 600.0, SIZE_MIN, SIZE_MAX);
    s.w = w;
    s.h = h;
    s.ground = if h * 0.8 < h - 60.0 { h * 0.8 } else { h - 60.0 };
    if s.ground < h * 0.5 {
        s.ground = h * 0.5; // tiny viewports: keep the crust on screen
    }
    s.sx = w * 0.5;
    s.sy = s.ground + 16.0;
    s.rng = seed | 1; // xorshift must never be seeded with 0
    s.t = 0.0;
    s.reset();
    s.retract_to = 0;
}

/// Advance the simulation by `dt` seconds and rebuild the draw list.
/// Returns how many floats of the draw list are valid (always ≥ 1).
/// A non-finite pointer position is treated as "pointer gone".
#[unsafe(no_mangle)]
pub extern "C" fn tick(dt: f32, mx: f32, my: f32, active: i32) -> usize {
    // SAFETY: the only reference to WORLD during this call (see `world`).
    let s = unsafe { world() };
    if s.n == 0 {
        // Called before `init`: there is no world yet. Emit an empty list
        // instead of touching uninitialised indices.
        s.out[0] = CMD_END;
        s.on = 1;
        return s.on;
    }
    let dt = sanitize(dt, 0.0, 0.0, 0.05);
    let pointer_ok = mx.is_finite() && my.is_finite();
    let mx = sanitize(mx, 0.0, -COORD_LIMIT, COORD_LIMIT);
    let my = sanitize(my, 0.0, -COORD_LIMIT, COORD_LIMIT);
    s.t += dt;
    if s.t > CLOCK_WRAP {
        s.t -= CLOCK_WRAP;
    }
    s.update(dt, mx, my, active != 0 && pointer_ok);
    s.heal();
    s.emit();
    s.on
}

#[unsafe(no_mangle)]
pub extern "C" fn out_ptr() -> *const f32 {
    // SAFETY: the only reference to WORLD during this call (see `world`).
    unsafe { world() }.out.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn ground_y() -> f32 {
    // SAFETY: the only reference to WORLD during this call (see `world`).
    unsafe { world() }.ground
}
