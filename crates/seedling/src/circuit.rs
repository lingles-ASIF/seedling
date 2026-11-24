//! Circuit-board trees (Cargo feature `circuit`).
//!
//! A straight trunk rises from the seed toward the pointer's height. When the
//! pointer moves beside the trunk, a branch leaves the trunk at that height and
//! routes toward the pointer like a printed-circuit trace: straight runs joined
//! by 45° turns only, never curves. Short side traces fork off at 45° as it
//! grows, and every trace ends in a leaf. The branch being driven is vivid;
//! branches left alone fade into a faint outline, the "fog".
//!
//! This module adds methods to [`Plant`]; its state lives in [`Circuit`].

use crate::math::{clamp, dist, fl, smooth};
use crate::{Drive, Leaf, LeafKind, Plant, Renderer, StrokeKind, StrokePoint, Style};

/// Most traces (branches plus their side traces) a circuit tree keeps.
pub const MAX_BRANCHES: usize = 160;
const MAX_CORNERS: usize = 12; // per trace, including its start and its tip
const MIN_SEG: f32 = 10.0; // px a trace runs straight before it may turn
const TURN_MARGIN: f32 = 0.12; // a turn must aim this much better (cosine) to happen
const STEP: f32 = 2.0; // px per integration step
const FOG_TIME: f32 = 3.0; // s for an idle branch to fade into the fog
const TRUNK_FOG_REACH: f32 = 160.0; // px of trunk around the pointer that stay vivid
const SIDE_DEADZONE: f32 = 14.0; // px either side of the trunk where no branch starts
const LET_GO: f32 = 0.25; // s without being driven before a branch counts as finished
const MIN_BRANCH: f32 = 22.0; // px every branch grows, even if the pointer moves on

/// The 8 trace directions, 45° apart, as unit vectors in screen space (y down).
/// Index: 0 E, 1 SE, 2 S, 3 SW, 4 W, 5 NW, 6 N, 7 NE.
const DIRS: [(f32, f32); 8] = [
    (1.0, 0.0),
    (core::f32::consts::FRAC_1_SQRT_2, core::f32::consts::FRAC_1_SQRT_2),
    (0.0, 1.0),
    (-core::f32::consts::FRAC_1_SQRT_2, core::f32::consts::FRAC_1_SQRT_2),
    (-1.0, 0.0),
    (-core::f32::consts::FRAC_1_SQRT_2, -core::f32::consts::FRAC_1_SQRT_2),
    (0.0, -1.0),
    (core::f32::consts::FRAC_1_SQRT_2, -core::f32::consts::FRAC_1_SQRT_2),
];

/// One trace: a branch from the trunk, or a short side trace from a branch.
#[derive(Clone, Copy)]
pub struct Branch {
    corners: [[f32; 2]; MAX_CORNERS], // [0] = start; [nc - 1] = current tip
    nc: usize,
    dir: usize,     // index into DIRS
    seg: f32,       // length of the current straight run
    len: f32,       // grown path length
    shown: f32,     // drawn length (below `len` only while withering)
    max_len: f32,   // longest this trace may grow
    tx: f32,        // target the trace routes toward
    ty: f32,
    parent: usize,  // 0 = the trunk; k + 1 = a side trace of branch k
    at: f32,        // side traces: parent's path length where this one starts
    side: f32,      // -1 left of the trunk, +1 right
    bucket: i32,    // branches: height slot on the trunk
    growing: bool,
    leaf_g: f32,    // leaf growth 0..1
    leaf_size: f32, // leaf size scale
    link: u8,       // 0 = none, k + 1 = link k
    active_t: f32,  // clock time when last driven (for the fog)
    next_twig: f32, // path length at which to sprout the next side trace
    rise: f32,      // branches: length of the first, upward 45° run
    lift: f32,      // branches: how far above its start the branch aims, px
}

const BRANCH0: Branch = Branch {
    corners: [[0.0; 2]; MAX_CORNERS],
    nc: 0,
    dir: 0,
    seg: 0.0,
    len: 0.0,
    shown: 0.0,
    max_len: 0.0,
    tx: 0.0,
    ty: 0.0,
    parent: 0,
    at: 0.0,
    side: 0.0,
    bucket: 0,
    growing: false,
    leaf_g: 0.0,
    leaf_size: 0.0,
    link: 0,
    active_t: 0.0,
    next_twig: 0.0,
    rise: 0.0,
    lift: 0.0,
};

/// State of a circuit-board tree. All-zero when blank.
pub struct Circuit {
    trunk: f32,   // grown trunk height above the crust, px
    focus_y: f32, // pointer height last frame (the vivid part of the trunk)
    branches: [Branch; MAX_BRANCHES],
    nb: usize,
    withering: bool,
}

impl Circuit {
    #[allow(clippy::large_stack_arrays)] // const-evaluated inside Plant::blank
    pub const BLANK: Self =
        Self { trunk: 0.0, focus_y: 0.0, branches: [BRANCH0; MAX_BRANCHES], nb: 0, withering: false };
}

impl Plant {
    // ------------------------------------------------------------- state

    pub(crate) const fn circuit_reset(&mut self) {
        self.circuit.trunk = 0.0;
        self.circuit.focus_y = 0.0;
        self.circuit.nb = 0;
        self.circuit.withering = false;
    }

    pub(crate) fn circuit_has_plant(&self) -> bool {
        self.circuit.trunk > 0.5 || self.circuit.nb > 0
    }

    pub(crate) const fn circuit_ok(&self) -> bool {
        self.circuit.trunk.is_finite() && self.circuit.focus_y.is_finite()
    }

    /// Tallest the trunk may grow, px above the crust.
    fn trunk_cap(&self) -> f32 {
        let room = self.ground - 24.0; // leave a margin below the top edge
        let want = self.config().trunk_height;
        if want > 0.0 && want < room { want } else { room.max(0.0) }
    }

    // ---------------------------------------------------------- simulation

    pub(crate) fn circuit_update(&mut self, dt: f32, drive: Drive) {
        match drive {
            Drive::At(x, y) => {
                let near_seed = dist(x, y, self.sx, self.ground) < crate::SEED_ZONE;
                if near_seed {
                    self.circuit.withering = true;
                } else {
                    self.circuit.withering = false;
                    self.circuit.focus_y = y;
                    self.circuit_drive(dt, x, y);
                }
            }
            Drive::Away => self.circuit.withering = true,
            Drive::Rest => {}
        }

        let speed = self.config().grow_max;
        if self.circuit.withering {
            self.circuit_wither(dt, speed * 1.6);
            return;
        }
        // Not withering: re-sprout anything a cut-short wither left behind,
        // grow side traces on their own, and unfold leaves on finished traces.
        // (Indexing, not iteration: side traces may be added along the way.)
        let t = self.t;
        let mut i = 0;
        while i < self.circuit.nb {
            let b = &mut self.circuit.branches[i];
            // A branch the pointer has left is finished: it unfolds its leaf
            // (and shows its link). Driving it outward again resumes growth.
            let idle = t - b.active_t;
            if b.parent == 0 && b.growing && !(0.0..LET_GO).contains(&idle) {
                b.growing = false;
            }
            // Too short to read as a branch: let it finish a minimum length
            // on its own, straight on, so no leaf is left jammed at the trunk.
            if b.parent == 0 && !b.growing && b.len < MIN_BRANCH.min(b.max_len) && b.shown >= b.len {
                let tip = b.corners[b.nc - 1];
                let more = MIN_BRANCH - b.len;
                b.tx = tip[0] + DIRS[b.dir].0 * more;
                b.ty = tip[1] + DIRS[b.dir].1 * more;
                b.growing = true;
                self.circuit_extend(i, dt * speed * 0.6);
                self.circuit.branches[i].growing = false;
            }
            let b = &mut self.circuit.branches[i];
            if b.shown < b.len {
                b.shown = (b.shown + speed * 1.6 * dt).min(b.len);
            }
            if b.parent > 0 && b.growing {
                self.circuit_extend(i, dt * speed * 0.6);
            }
            let b = &mut self.circuit.branches[i];
            if b.growing {
                b.leaf_g = (b.leaf_g - dt * 3.0).max(0.0);
            } else {
                b.leaf_g = (b.leaf_g + dt * 1.6).min(1.0);
            }
            i += 1;
        }
    }

    /// The pointer is at (`mx`, `my`): raise the trunk toward it, and start
    /// or extend the branch at its height on its side.
    fn circuit_drive(&mut self, dt: f32, mx: f32, my: f32) {
        let cfg = self.config();
        let speed = cfg.grow_max;

        // 1. The trunk rises toward the pointer's height (a little above it).
        let want = clamp(self.ground - my + 20.0, 0.0, self.trunk_cap());
        if want > self.circuit.trunk {
            self.circuit.trunk = (self.circuit.trunk + speed * 0.8 * dt).min(want);
        }

        // 2. A branch at the pointer's height, on the pointer's side.
        let dx = mx - self.sx;
        if dx.abs() < SIDE_DEADZONE {
            return; // pointer on the trunk: the trunk is what grows
        }
        let side: f32 = if dx > 0.0 { 1.0 } else { -1.0 };
        let spacing = cfg.branch_spacing;
        let top = self.ground - self.circuit.trunk;
        // Height slot, counted up from the crust; `my` is clamped to
        // ±COORD_LIMIT, so the result is far inside i32.
        #[allow(clippy::cast_possible_truncation)]
        let bucket = ((self.ground - my) / spacing + 0.5) as i32;
        let attach_y = self.ground - fl(usize::try_from(bucket.max(0)).unwrap_or(0)) * spacing;
        if bucket < 1 || attach_y < top + 8.0 {
            return; // below the first slot, or above what the trunk has reached
        }

        let found = (0..self.circuit.nb).find(|&i| {
            let b = &self.circuit.branches[i];
            b.parent == 0 && b.bucket == bucket && b.side.to_bits() == side.to_bits()
        });
        let i = match found {
            Some(i) => i,
            None => match self.circuit_spawn(attach_y, side, bucket) {
                Some(i) => i,
                None => return, // out of trace slots
            },
        };

        let t = self.t;
        let b = &mut self.circuit.branches[i];
        b.active_t = t;
        b.tx = mx;
        // Branches reach up toward the light: aim at least `lift` px above
        // where the branch leaves the trunk.
        b.ty = my.min(b.corners[0][1] - b.lift);
        let tip = b.corners[b.nc - 1];
        let reach = (mx - tip[0]) * side; // how far past the tip, outward
        if !b.growing && reach > 8.0 && b.len < b.max_len - 1.0 {
            b.growing = true; // the pointer moved further out: extend again
        }
        if b.growing {
            self.circuit_extend(i, speed * dt);
        }
    }

    /// Start a new branch from the trunk at `attach_y`. Returns its index.
    fn circuit_spawn(&mut self, attach_y: f32, side: f32, bucket: i32) -> Option<usize> {
        if self.circuit.nb >= MAX_BRANCHES {
            return None;
        }
        let start = [self.sx + side * 1.5, attach_y];
        let max_len = self.config().branch_length * self.range(0.85, 1.0);
        let b = Branch {
            corners: {
                let mut c = [[0.0; 2]; MAX_CORNERS];
                c[0] = start;
                c[1] = start;
                c
            },
            nc: 2,
            dir: if side > 0.0 { 7 } else { 5 }, // NE / NW: up and out
            max_len,
            side,
            bucket,
            rise: self.range(10.0, 26.0),
            lift: self.range(12.0, 44.0),
            growing: true,
            leaf_size: self.range(0.9, 1.15),
            link: self.take_link(),
            active_t: self.t,
            next_twig: self.range(26.0, 40.0),
            ..BRANCH0
        };
        let i = self.circuit.nb;
        self.circuit.branches[i] = b;
        self.circuit.nb += 1;
        Some(i)
    }

    /// Start a short side trace at the tip of branch `p`, angled 45° off it.
    fn circuit_twig(&mut self, p: usize) {
        if self.circuit.nb >= MAX_BRANCHES {
            return;
        }
        let parent = self.circuit.branches[p];
        let tip = parent.corners[parent.nc - 1];
        // Mostly toward the light (upward), sometimes downward.
        let up = self.rnd() < 0.7;
        let (cw, ccw) = ((parent.dir + 1) % 8, (parent.dir + 7) % 8);
        let dir = if (DIRS[ccw].1 < DIRS[cw].1) == up { ccw } else { cw };
        if DIRS[dir].0 * parent.side < -0.1 {
            return; // would point back toward the trunk
        }
        let len = self.range(10.0, 22.0);
        let b = Branch {
            corners: {
                let mut c = [[0.0; 2]; MAX_CORNERS];
                c[0] = tip;
                c[1] = tip;
                c
            },
            nc: 2,
            dir,
            max_len: len,
            tx: DIRS[dir].0 * len + tip[0],
            ty: DIRS[dir].1 * len + tip[1],
            parent: p + 1,
            at: parent.len,
            side: parent.side,
            growing: true,
            leaf_size: self.range(0.6, 0.85),
            link: self.take_link(),
            active_t: self.t,
            ..BRANCH0
        };
        let i = self.circuit.nb;
        self.circuit.branches[i] = b;
        self.circuit.nb += 1;
    }

    /// Grow trace `i` by up to `amount` px toward its target, turning only in
    /// 45° steps and only after a straight run of at least `MIN_SEG`.
    fn circuit_extend(&mut self, i: usize, amount: f32) {
        let (w, ground) = (self.w, self.ground);
        let mut rem = amount;
        let mut sprout = false;
        {
            let b = &mut self.circuit.branches[i];
            while rem > 0.0 && b.growing {
                let tip = b.corners[b.nc - 1];
                let (ux, uy) = (b.tx - tip[0], b.ty - tip[1]);
                let d = crate::math::sqrt(ux * ux + uy * uy);
                if d < 3.0 || b.len >= b.max_len {
                    b.growing = false;
                    break;
                }
                // Side traces run straight; branches may turn by 45°.
                let first_run = if b.nc == 2 { b.rise } else { 0.0 };
                if b.parent == 0 && b.seg >= MIN_SEG.max(first_run) {
                    let aim = |k: usize| (DIRS[k].0 * ux + DIRS[k].1 * uy) / d;
                    let mut best = b.dir;
                    let mut best_aim = aim(b.dir) + TURN_MARGIN;
                    for k in [(b.dir + 1) % 8, (b.dir + 7) % 8] {
                        let outward = DIRS[k].0 * b.side >= -0.1; // never back toward the trunk
                        let not_down = k != 2; // no trace drops straight down
                        if outward && not_down && aim(k) > best_aim {
                            best = k;
                            best_aim = aim(k);
                        }
                    }
                    if best != b.dir {
                        if b.nc >= MAX_CORNERS {
                            b.growing = false; // out of corners: finish here
                            break;
                        }
                        b.corners[b.nc] = tip; // the old tip becomes a corner
                        b.nc += 1;
                        b.dir = best;
                        b.seg = 0.0;
                    }
                }
                let step = rem.min(STEP).min(d).min(b.max_len - b.len);
                let (nx, ny) = (tip[0] + DIRS[b.dir].0 * step, tip[1] + DIRS[b.dir].1 * step);
                if nx < 4.0 || nx > w - 4.0 || ny < 4.0 || ny > ground - 6.0 {
                    b.growing = false; // reached the edge of the window or the soil
                    break;
                }
                b.corners[b.nc - 1] = [nx, ny];
                b.seg += step;
                b.len += step;
                b.shown = b.len;
                rem -= step;
                if b.parent == 0 && b.len >= b.next_twig {
                    sprout = true;
                    break;
                }
            }
        }
        if sprout {
            let gap = self.range(28.0, 52.0);
            self.circuit.branches[i].next_twig += gap;
            self.circuit_twig(i);
            if rem > 0.0 {
                self.circuit_extend(i, rem);
            }
        }
    }

    /// Wither: traces shrink back, then the trunk sinks; then start over.
    fn circuit_wither(&mut self, dt: f32, speed: f32) {
        let mut any = false;
        for b in &mut self.circuit.branches[..self.circuit.nb] {
            b.growing = false;
            b.leaf_g = (b.leaf_g - dt * 4.0).max(0.0);
            b.shown = (b.shown - speed * dt).max(0.0);
            any |= b.shown > 0.0;
        }
        if !any {
            self.circuit.trunk = (self.circuit.trunk - speed * dt).max(0.0);
            if self.circuit.trunk <= 0.0 && self.circuit.nb > 0 {
                self.circuit.nb = 0;
                self.links_used = 0; // every trace is gone, so are their links
            }
        }
    }

    // ------------------------------------------------------------- drawing

    /// Fog: 1 while a branch is being driven, fading to `Config::fog`.
    fn circuit_alpha(&self, b: &Branch) -> f32 {
        let idle = self.t - b.active_t;
        let idle = if idle < 0.0 { FOG_TIME } else { idle }; // clock wrapped
        let fog = self.config().fog;
        fog + (1.0 - fog) * smooth(1.0 - idle / FOG_TIME)
    }

    fn trunk_alpha(&self, y: f32) -> f32 {
        let fog = self.config().fog.max(0.3); // the trunk always stays hinted
        fog + (1.0 - fog) * smooth(1.0 - (y - self.circuit.focus_y).abs() / TRUNK_FOG_REACH)
    }

    /// Trace `b` is visible if it has length and, for a side trace, its parent
    /// has (re)grown past the point where it starts.
    fn circuit_visible(&self, b: &Branch) -> bool {
        b.shown > 0.0 && (b.parent == 0 || self.circuit.branches[b.parent - 1].shown >= b.at)
    }

    pub(crate) fn circuit_draw<R: Renderer + ?Sized>(&self, r: &mut R) {
        // The trunk: a vertical trace, vivid around the pointer's height.
        // Points every 24 px give the fog room to vary along the trunk.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = clamp(self.circuit.trunk / 24.0, 1.0, 400.0) as usize;
        r.stroke_begin(StrokeKind::Trunk, steps + 2);
        r.stroke_point(StrokePoint { x: self.sx, y: self.sy, width: 3.2, alpha: self.trunk_alpha(self.sy) });
        for k in 0..=steps {
            let y = self.ground - self.circuit.trunk * fl(k) / fl(steps);
            let width = 3.2 - 1.0 * fl(k) / fl(steps);
            r.stroke_point(StrokePoint { x: self.sx, y, width, alpha: self.trunk_alpha(y) });
        }

        // Traces, then leaves on top.
        for b in &self.circuit.branches[..self.circuit.nb] {
            if !self.circuit_visible(b) {
                continue;
            }
            let alpha = self.circuit_alpha(if b.parent > 0 { &self.circuit.branches[b.parent - 1] } else { b });
            let width = if b.parent > 0 { 1.1 } else { 1.9 };
            let (n, _, _) = walk(b, b.shown, |_| {});
            r.stroke_begin(StrokeKind::Trace, n);
            walk(b, b.shown, |p| r.stroke_point(StrokePoint { x: p[0], y: p[1], width, alpha }));
        }
        for b in &self.circuit.branches[..self.circuit.nb] {
            if !self.circuit_visible(b) || b.leaf_g <= 0.02 {
                continue;
            }
            let alpha = self.circuit_alpha(if b.parent > 0 { &self.circuit.branches[b.parent - 1] } else { b });
            let (_, end, dir) = walk(b, b.shown, |_| {});
            // Birch leaves hang a little below the trace direction.
            let (dx, dy) = crate::math::norm(DIRS[dir].0, DIRS[dir].1 + 0.55, 0.0, 1.0);
            let leaf_kind = match self.config().style {
                Style::Vine | Style::CircuitBirch => LeafKind::Birch,
            };
            r.leaf(Leaf {
                x: end[0],
                y: end[1],
                dir_x: dx,
                dir_y: dy,
                size: 9.0 * b.leaf_size * smooth(b.leaf_g),
                alpha,
                link: self.link_code(b.link),
                kind: leaf_kind,
            });
        }
    }
}

/// Walk trace `b` from its start for `len` px, calling `emit` on each point
/// (start, corners, end). Returns (points emitted, end point, final direction).
fn walk(b: &Branch, len: f32, mut emit: impl FnMut([f32; 2])) -> (usize, [f32; 2], usize) {
    let mut n = 1;
    emit(b.corners[0]);
    let mut left = len;
    let mut dir = if b.side > 0.0 { 0 } else { 4 };
    for k in 1..b.nc {
        let (a, c) = (b.corners[k - 1], b.corners[k]);
        let seg = dist(a[0], a[1], c[0], c[1]);
        if seg > 1e-3 {
            dir = dir_of(c[0] - a[0], c[1] - a[1]);
        }
        if left <= seg || k == b.nc - 1 {
            let f = if seg > 1e-3 { (left / seg).min(1.0) } else { 1.0 };
            let end = [a[0] + (c[0] - a[0]) * f, a[1] + (c[1] - a[1]) * f];
            emit(end);
            return (n + 1, end, dir);
        }
        emit(c);
        n += 1;
        left -= seg;
    }
    (n, b.corners[0], dir)
}

/// The DIRS index closest to the vector (`x`, `y`).
fn dir_of(x: f32, y: f32) -> usize {
    let mut best = 0;
    let mut best_dot = f32::MIN;
    for (k, d) in DIRS.iter().enumerate() {
        let dot = d.0 * x + d.1 * y;
        if dot > best_dot {
            best = k;
            best_dot = dot;
        }
    }
    best
}
