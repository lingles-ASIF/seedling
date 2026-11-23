//! Behaviour tests for the growth engine. They run on the host with
//! `cargo test -p seedling`.

// Exact float comparisons are deliberate here: determinism and clamping are
// what these tests check.
#![allow(clippy::float_cmp)]

use super::*;

/// Records everything a plant draws and checks the stream is well formed:
/// every number finite, and every stroke followed by exactly the promised
/// number of points.
#[derive(Default)]
struct Recorder {
    seeds: usize,
    stems: usize,
    twigs: usize,
    leaves: usize,
    flowers: usize,
    points: usize,
    expecting: usize,
    max_x: f32,
    min_y: f32,
}

fn finite(vals: &[f32]) {
    for v in vals {
        assert!(v.is_finite(), "non-finite value in draw output: {v}");
    }
}

impl Recorder {
    fn not_mid_stroke(&self) {
        assert_eq!(self.expecting, 0, "a shape arrived before the current stroke was complete");
    }
}

impl Renderer for Recorder {
    fn seed(&mut self, s: Seed) {
        self.not_mid_stroke();
        finite(&[s.x, s.y, s.pulse, s.prominence]);
        self.seeds += 1;
        self.min_y = f32::MAX;
    }
    fn stroke_begin(&mut self, kind: StrokeKind, points: usize) {
        self.not_mid_stroke();
        assert!(points >= 1);
        self.expecting = points;
        match kind {
            StrokeKind::Stem => self.stems += 1,
            StrokeKind::Twig => self.twigs += 1,
        }
    }
    fn stroke_point(&mut self, p: StrokePoint) {
        assert!(self.expecting > 0, "stroke point without stroke_begin");
        finite(&[p.x, p.y, p.width, p.alpha]);
        assert!((0.0..=1.0).contains(&p.alpha), "alpha out of range: {}", p.alpha);
        self.expecting -= 1;
        self.points += 1;
        self.max_x = self.max_x.max(p.x);
        self.min_y = self.min_y.min(p.y);
    }
    fn leaf(&mut self, l: Leaf) {
        self.not_mid_stroke();
        finite(&[l.x, l.y, l.dir_x, l.dir_y, l.size, l.alpha]);
        self.leaves += 1;
    }
    fn flower(&mut self, f: Flower) {
        self.not_mid_stroke();
        finite(&[f.x, f.y, f.dir_x, f.dir_y, f.size, f.open, f.alpha]);
        assert!((0.0..=1.0).contains(&f.open));
        self.flowers += 1;
    }
}

fn record(p: &Plant) -> Recorder {
    let mut r = Recorder::default();
    p.draw(&mut r);
    r.not_mid_stroke();
    r
}

/// Drive the pointer in a slow upward S-curve for `frames` frames at 60 fps.
fn grow(p: &mut Plant, frames: usize) {
    for k in 0..frames {
        let t = fl(k) / fl(frames);
        let x = 400.0 + 200.0 * sin(t * 5.0);
        let y = 460.0 - 380.0 * t;
        p.step(1.0 / 60.0, Some((x, y)));
    }
}

#[test]
fn blank_plant_is_inert() {
    let mut p = Plant::blank();
    p.step(0.016, Some((100.0, 100.0)));
    let r = record(&p);
    assert_eq!(r.seeds + r.stems + r.leaves + r.flowers, 0);
    assert!(!p.has_plant());
}

#[test]
fn fresh_plant_shows_only_the_seed() {
    let p = Plant::new(800.0, 600.0, 1);
    let r = record(&p);
    assert_eq!((r.seeds, r.stems, r.leaves, r.flowers), (1, 0, 0, 0));
    assert!(p.ground_y() > 0.0 && p.ground_y() < 600.0);
    assert!(p.seed_position().1 > p.ground_y(), "seed sits below the crust");
}

#[test]
fn grows_toward_the_pointer_with_leaves() {
    let mut p = Plant::new(800.0, 600.0, 7);
    grow(&mut p, 600);
    let r = record(&p);
    assert!(p.has_plant());
    assert_eq!(r.stems, 1);
    assert!(r.twigs > 3, "expected side twigs, got {}", r.twigs);
    assert!(r.leaves + r.flowers > 3);
    assert!(r.min_y < 200.0, "stem should have climbed toward the pointer (top at {})", r.min_y);
}

#[test]
fn same_seed_same_plant_different_seed_different_plant() {
    let (mut a, mut b, mut c) = (Plant::new(800.0, 600.0, 99), Plant::new(800.0, 600.0, 99), Plant::new(800.0, 600.0, 100));
    for p in [&mut a, &mut b, &mut c] {
        grow(p, 400);
    }
    let (ra, rb, rc) = (record(&a), record(&b), record(&c));
    assert_eq!((ra.points, ra.leaves, ra.flowers), (rb.points, rb.leaves, rb.flowers));
    assert!(ra.max_x.to_bits() == rb.max_x.to_bits(), "identical inputs must give identical plants");
    assert!(ra.max_x.to_bits() != rc.max_x.to_bits() || ra.points != rc.points);
}

#[test]
fn withers_home_when_the_pointer_leaves() {
    let mut p = Plant::new(800.0, 600.0, 3);
    grow(&mut p, 400);
    assert!(p.has_plant());
    for _ in 0..180 {
        p.step(1.0 / 60.0, None); // 3 s with no pointer
    }
    assert!(!p.has_plant(), "plant should have withered back to the seed");
    let r = record(&p);
    assert_eq!((r.stems, r.leaves), (0, 0));
}

#[test]
fn returning_to_the_seed_buries_it() {
    let mut p = Plant::new(800.0, 600.0, 5);
    grow(&mut p, 300);
    let (sx, _) = p.seed_position();
    for _ in 0..240 {
        p.step(1.0 / 60.0, Some((sx, p.ground_y())));
    }
    assert!(!p.has_plant());
}

#[test]
fn hostile_inputs_never_poison_the_plant() {
    let mut p = Plant::new(800.0, 600.0, 11);
    grow(&mut p, 120);
    let bad = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX, -f32::MAX, 1e30, -1e30];
    for &v in &bad {
        p.step(v, Some((v, v)));
        p.step(0.016, Some((v, 300.0)));
        p.step(0.016, Some((300.0, v)));
        let _ = record(&p); // asserts every value is finite
    }
    grow(&mut p, 300);
    assert!(p.has_plant(), "plant must keep working after hostile input");
    let _ = record(&p);
}

#[test]
fn hostile_viewport_sizes_are_clamped() {
    for &(w, h) in &[(f32::NAN, f32::NAN), (0.0, 0.0), (-5.0, 1e9), (f32::INFINITY, 2.0)] {
        let mut p = Plant::new(w, h, 1);
        grow(&mut p, 120);
        let _ = record(&p);
        assert!(p.ground_y().is_finite());
    }
}

#[test]
fn config_is_clamped_and_applied() {
    let c = Config {
        max_turn: f32::NAN,
        grow_min: -1.0,
        grow_max: 1.0, // below grow_min: raised to it
        fog: 7.0,
        flower_chance: 1.0, // every twig ends in a flower
        ..Config::default()
    };
    let mut p = Plant::new(800.0, 600.0, 2);
    p.set_config(c);
    let got = p.config();
    assert_eq!(got.max_turn, Config::DEFAULT.max_turn);
    assert_eq!((got.grow_min, got.grow_max), (10.0, 10.0));
    assert_eq!(got.fog, 1.0);
    grow(&mut p, 600);
    let r = record(&p);
    assert_eq!(r.leaves, 0, "flower_chance = 1 means no leaves");
    assert!(r.flowers > 0);
}

#[test]
fn reaches_capacity_without_overflowing() {
    let mut p = Plant::new(2000.0, 2000.0, 3);
    for k in 0..30_000 {
        let a = fl(k) * 0.01;
        p.step(0.05, Some((1000.0 + 900.0 * cos_approx(a), 900.0 + 800.0 * sin(a * 1.3))));
    }
    let r = record(&p);
    assert!(r.points <= MAX_STEM_POINTS + 1 + MAX_TWIGS * 12);
}

fn cos_approx(a: f32) -> f32 {
    math::cos(a)
}
