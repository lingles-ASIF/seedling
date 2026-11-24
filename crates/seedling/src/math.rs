//! Small, dependency-free math helpers.
//!
//! `core` (the `no_std` standard library) has no `sin`, `cos` or `sqrt` on
//! stable Rust, so they are written out here. They are deterministic and give
//! the same results on every platform, which keeps plants reproducible.

use core::f32::consts::{PI, TAU};

pub fn sqrt(x: f32) -> f32 {
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

pub fn sin(x: f32) -> f32 {
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
pub fn cos(x: f32) -> f32 {
    sin(x + PI * 0.5)
}

#[inline]
pub const fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

/// Index or count → float. Every count in this engine is bounded well below
/// 2^24, so the conversion is exact.
#[inline]
#[allow(clippy::cast_precision_loss)]
pub const fn fl(n: usize) -> f32 {
    n as f32
}

/// Replace NaN/±Infinity with a default, then clamp into [lo, hi].
#[inline]
pub const fn sanitize(x: f32, default: f32, lo: f32, hi: f32) -> f32 {
    if x.is_finite() { clamp(x, lo, hi) } else { default }
}

#[inline]
pub const fn smooth(x: f32) -> f32 {
    let u = clamp(x, 0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

#[inline]
pub fn dist(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    sqrt(dx * dx + dy * dy)
}

#[inline]
pub fn norm(x: f32, y: f32, fx: f32, fy: f32) -> (f32, f32) {
    let l = sqrt(x * x + y * y);
    if l < 1e-6 { (fx, fy) } else { (x / l, y / l) }
}

#[inline]
pub fn rotate(x: f32, y: f32, a: f32) -> (f32, f32) {
    // Screen space (y points down): positive angle turns clockwise on screen.
    let (c, s) = (cos(a), sin(a));
    (x * c - y * s, x * s + y * c)
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact comparisons are the point of these tests
mod tests {
    use super::*;

    #[test]
    fn sqrt_matches_std() {
        for &x in &[1e-6_f32, 0.25, 1.0, 2.0, 144.0, 1e6, 3.0e10] {
            let err = (sqrt(x) - x.sqrt()).abs() / x.sqrt();
            assert!(err < 1e-6, "sqrt({x}) relative error {err}");
        }
        assert_eq!((sqrt(0.0), sqrt(-4.0)), (0.0, 0.0));
    }

    #[test]
    fn sin_cos_match_std() {
        for k in 0..2700_usize {
            let x = fl(k) * 0.037 - 50.0; // -50..50
            assert!((sin(x) - x.sin()).abs() < 2e-4, "sin({x})");
            assert!((cos(x) - x.cos()).abs() < 2e-4, "cos({x})");
        }
    }

    #[test]
    fn sanitize_replaces_non_finite() {
        assert_eq!(sanitize(f32::NAN, 7.0, 0.0, 10.0), 7.0);
        assert_eq!(sanitize(f32::INFINITY, 7.0, 0.0, 10.0), 7.0);
        assert_eq!(sanitize(42.0, 7.0, 0.0, 10.0), 10.0);
    }
}
