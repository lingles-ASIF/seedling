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
pub struct Recorder {
    seeds: usize,
    stems: usize,
    twigs: usize,
    leaves: usize,
    flowers: usize,
    points: usize,
    expecting: usize,
    max_x: f32,
    min_y: f32,
    links: Vec<u8>,
    /// Every stroke, with its points, in drawing order.
    pub paths: Vec<(StrokeKind, Vec<(f32, f32)>)>,
    /// Every leaf's kind.
    pub leaf_kinds: Vec<LeafKind>,
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
            _ => {}
        }
        self.paths.push((kind, Vec::with_capacity(points)));
    }
    fn stroke_point(&mut self, p: StrokePoint) {
        assert!(self.expecting > 0, "stroke point without stroke_begin");
        finite(&[p.x, p.y, p.width, p.alpha]);
        assert!((0.0..=1.0).contains(&p.alpha), "alpha out of range: {}", p.alpha);
        self.expecting -= 1;
        self.points += 1;
        if let Some((_, pts)) = self.paths.last_mut() {
            pts.push((p.x, p.y));
        }
        self.max_x = self.max_x.max(p.x);
        self.min_y = self.min_y.min(p.y);
    }
    fn leaf(&mut self, l: Leaf) {
        self.not_mid_stroke();
        finite(&[l.x, l.y, l.dir_x, l.dir_y, l.size, l.alpha]);
        self.leaves += 1;
        self.links.extend(l.link);
        self.leaf_kinds.push(l.kind);
    }
    fn flower(&mut self, f: Flower) {
        self.not_mid_stroke();
        finite(&[f.x, f.y, f.dir_x, f.dir_y, f.size, f.open, f.alpha]);
        assert!((0.0..=1.0).contains(&f.open));
        self.flowers += 1;
        self.links.extend(f.link);
    }
}

pub fn record(p: &Plant) -> Recorder {
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

fn with(links: u8, persist: bool) -> Config {
    Config { links, persist, ..Config::default() }
}

/// Links currently on live twigs, from the plant's own state.
fn live_links(p: &Plant) -> Vec<u8> {
    p.twigs[..p.nt].iter().filter_map(|t| p.link_of(t)).collect()
}

fn assert_distinct(links: &[u8], below: u8) {
    let mut seen = [false; 256];
    for &l in links {
        assert!(l < below, "link {l} out of range (links = {below})");
        assert!(!seen[l as usize], "link {l} handed out twice: {links:?}");
        seen[l as usize] = true;
    }
}

#[test]
fn no_links_by_default() {
    let mut p = Plant::new(800.0, 600.0, 7);
    grow(&mut p, 600);
    assert!(record(&p).links.is_empty());
}

#[test]
fn links_are_handed_out_in_growth_order() {
    let mut p = Plant::new(800.0, 600.0, 7);
    p.set_config(with(3, false));
    grow(&mut p, 600);
    assert!(p.nt > 3, "need more twigs than links for this test");
    // Twigs are stored in creation order while nothing has withered.
    let first: Vec<u8> = p.twigs[..3].iter().map(|t| p.link_of(t).expect("first three twigs are linked")).collect();
    assert_eq!(first, vec![0, 1, 2]);
    assert!(p.twigs[3..p.nt].iter().all(|t| p.link_of(t).is_none()), "only 3 links exist");
    let drawn = record(&p).links;
    assert_distinct(&drawn, 3);
}

#[test]
fn withered_links_are_recycled_and_never_duplicated() {
    let mut p = Plant::new(800.0, 600.0, 21);
    p.set_config(with(5, false));
    for round in 0..4 {
        grow(&mut p, 500);
        let live = live_links(&p);
        assert_distinct(&live, 5);
        assert_eq!(live.len(), 5.min(p.nt), "round {round}: every link should be in use");
        // Wither most of the way home, then regrow: links must come back.
        for _ in 0..40 {
            p.step(1.0 / 60.0, None);
        }
        assert_distinct(&live_links(&p), 5);
    }
}

#[test]
fn persist_keeps_the_plant_when_the_pointer_leaves() {
    let mut p = Plant::new(800.0, 600.0, 3);
    p.set_config(with(4, true));
    grow(&mut p, 400);
    let before = (p.n, p.nt, live_links(&p));
    for _ in 0..600 {
        p.step(1.0 / 60.0, None); // 10 s away
    }
    assert_eq!((p.n, p.nt, live_links(&p)), before, "plant must stay exactly as grown");
    assert!(p.bloom > 0.9, "a resting plant blooms at the tip");
    // Returning to the seed still buries it.
    let (sx, _) = p.seed_position();
    for _ in 0..240 {
        p.step(1.0 / 60.0, Some((sx, p.ground_y())));
    }
    assert!(!p.has_plant());
}

#[test]
fn rest_holds_growth_still() {
    let mut p = Plant::new(800.0, 600.0, 9);
    grow(&mut p, 200);
    let (tip, n) = ((p.tipx, p.tipy), p.n);
    for _ in 0..120 {
        p.rest(1.0 / 60.0);
    }
    assert_eq!(((p.tipx, p.tipy), p.n), (tip, n), "rest must not grow or retract");
    let _ = record(&p);
}

#[test]
fn links_are_clamped() {
    let mut p = Plant::new(800.0, 600.0, 4);
    p.set_config(with(255, false));
    assert_eq!(p.config().links, MAX_LINKS);
    grow(&mut p, 900);
    assert_distinct(&record(&p).links, MAX_LINKS);
}

// ------------------------------------------------------------ circuit trees

#[cfg(feature = "circuit")]
mod circuit {
    use super::*;

    fn birch(links: u8, persist: bool) -> Plant {
        let mut p = Plant::new(1000.0, 700.0, 17);
        p.set_config(Config { style: Style::CircuitBirch, links, persist, ..Config::default() });
        p
    }

    /// Sweep the pointer up the window, swinging from side to side, so
    /// branches sprout at many heights on both sides of the trunk.
    fn drive(p: &mut Plant, frames: usize) {
        for k in 0..frames {
            let t = fl(k) / fl(frames);
            let x = 500.0 + 380.0 * sin(t * 23.0);
            let y = 520.0 - 460.0 * t;
            p.step(1.0 / 60.0, Some((x, y)));
        }
    }

    fn traces(r: &Recorder, kind: StrokeKind) -> Vec<&Vec<(f32, f32)>> {
        r.paths.iter().filter(|(k, _)| *k == kind).map(|(_, pts)| pts).collect()
    }

    fn length(pts: &[(f32, f32)]) -> f32 {
        pts.windows(2).map(|w| math::dist(w[0].0, w[0].1, w[1].0, w[1].1)).sum()
    }

    #[test]
    fn grows_a_vertical_trunk_and_many_branches() {
        let mut p = birch(0, false);
        drive(&mut p, 900);
        let r = record(&p);
        let trunk = traces(&r, StrokeKind::Trunk);
        assert_eq!(trunk.len(), 1);
        let (sx, _) = p.seed_position();
        assert!(trunk[0].iter().all(|&(x, _)| x == sx), "the trunk must be perfectly vertical");
        let top = trunk[0].iter().map(|&(_, y)| y).fold(f32::MAX, f32::min);
        assert!(top < 200.0, "trunk should have risen toward the pointer (top at {top})");
        assert!(traces(&r, StrokeKind::Trace).len() > 6, "expected branches and side traces");
        assert_eq!((r.stems, r.twigs), (0, 0), "no vine parts in a circuit tree");
    }

    #[test]
    fn traces_only_run_at_multiples_of_45_degrees() {
        let mut p = birch(0, false);
        drive(&mut p, 900);
        let r = record(&p);
        for pts in traces(&r, StrokeKind::Trace) {
            for w in pts.windows(2) {
                let (dx, dy) = ((w[1].0 - w[0].0).abs(), (w[1].1 - w[0].1).abs());
                if dx + dy < 0.5 {
                    continue; // zero-length (e.g. a trace that has just started)
                }
                let straight = dx < 0.05 || dy < 0.05;
                let diagonal = (dx - dy).abs() < 0.05 * (dx + dy).max(1.0);
                assert!(straight || diagonal, "segment ({dx}, {dy}) is not at a multiple of 45 degrees");
            }
        }
    }

    #[test]
    fn branches_respect_the_length_limit_and_never_turn_back() {
        let mut p = birch(0, false);
        let mut c = p.config();
        c.branch_length = 90.0;
        p.set_config(c);
        drive(&mut p, 900);
        let r = record(&p);
        let (sx, _) = p.seed_position();
        for pts in traces(&r, StrokeKind::Trace) {
            assert!(length(pts) <= 90.0 + 0.5, "trace longer than branch_length: {}", length(pts));
            let x0 = pts[0].0;
            let outward = if x0 >= sx { 1.0 } else { -1.0 };
            assert!(pts.iter().all(|&(x, _)| (x - x0) * outward >= -0.5), "a trace turned back toward the trunk");
        }
    }

    #[test]
    fn leaves_are_birch_and_links_are_unique() {
        let mut p = birch(6, false);
        drive(&mut p, 900);
        let r = record(&p);
        assert!(!r.leaf_kinds.is_empty());
        assert!(r.leaf_kinds.iter().all(|&k| k == LeafKind::Birch));
        assert_distinct(&r.links, 6);
        // All 6 are handed out; the branch being driven hides its leaf (like
        // a bud) until it stops growing, so up to one may be out of sight.
        assert_eq!(p.links_used.count_ones(), 6, "with plenty of leaves, all 6 links should be out");
        assert!(r.links.len() >= 5);
    }

    #[test]
    fn every_branch_left_alone_shows_its_leaf_and_link() {
        // Regression: a branch abandoned mid-growth used to stay "growing"
        // forever, hiding its leaf and its link. Found by the fuzz harness.
        let mut p = birch(8, true);
        drive(&mut p, 900);
        for _ in 0..90 {
            p.step(1.0 / 60.0, None); // pointer gone (persist): everything settles
        }
        let r = record(&p);
        let mut drawn = r.links.clone();
        drawn.sort_unstable();
        assert_eq!(drawn, (0..8).collect::<Vec<u8>>(), "every handed-out link must be visible");
    }

    #[test]
    fn withers_to_the_seed_or_persists() {
        for persist in [false, true] {
            let mut p = birch(3, persist);
            drive(&mut p, 600);
            assert!(p.has_plant());
            for _ in 0..600 {
                p.step(1.0 / 60.0, None);
            }
            assert_eq!(p.has_plant(), persist, "persist = {persist}");
            if !persist {
                assert_eq!(p.links_used, 0, "links are freed once the tree is gone");
            }
        }
    }

    #[test]
    fn changing_style_starts_over() {
        let mut p = birch(0, false);
        drive(&mut p, 400);
        assert!(p.has_plant());
        let mut c = p.config();
        c.style = Style::Vine;
        p.set_config(c);
        assert!(!p.has_plant(), "switching to the vine restarts from the seed");
        grow(&mut p, 300);
        assert!(record(&p).stems == 1, "and the vine then grows normally");
    }

    #[test]
    fn hostile_inputs_and_settings_are_safe() {
        let mut p = Plant::new(1000.0, 700.0, 5);
        p.set_config(Config {
            style: Style::CircuitBirch,
            branch_length: f32::NAN,
            trunk_height: f32::INFINITY,
            branch_spacing: -3.0,
            ..Config::default()
        });
        let c = p.config();
        assert_eq!(c.branch_length, Config::DEFAULT.branch_length);
        assert_eq!(c.trunk_height, 0.0, "infinite height means automatic");
        assert_eq!(c.branch_spacing, 8.0);
        drive(&mut p, 200);
        for &v in &[f32::NAN, f32::INFINITY, -f32::MAX, 1e30] {
            p.step(v, Some((v, v)));
            p.step(0.016, Some((v, 300.0)));
            p.step(0.016, Some((300.0, v)));
            p.rest(v);
            let _ = record(&p);
        }
        drive(&mut p, 300);
        assert!(p.has_plant());
        let _ = record(&p);
    }

    #[test]
    fn many_branches_stay_within_capacity() {
        let mut p = birch(64, false);
        let mut c = p.config();
        c.branch_spacing = 8.0;
        p.set_config(c);
        for k in 0..20_000 {
            let a = fl(k) * 0.013;
            p.step(0.05, Some((500.0 + 480.0 * math::cos(a * 3.1), 360.0 + 330.0 * sin(a))));
        }
        let r = record(&p);
        assert!(traces(&r, StrokeKind::Trace).len() <= MAX_BRANCHES);
        assert_distinct(&r.links, 64);
    }
}
