//! Browser wrapper for the Seedling growth engine.
//!
//! This crate is deliberately thin. All growth behaviour lives in the
//! `seedling` library crate; this one only:
//!
//! 1. keeps one [`Plant`] in WebAssembly memory,
//! 2. exposes five functions to the page (`init`, `configure`, `tick`,
//!    `out_ptr`, `ground_y`), and
//! 3. writes each frame's shapes into a flat `f32` draw list the page's
//!    JavaScript paints onto a canvas.
//!
//! Draw-list format (all `f32`): a sequence of commands, ending with `0`.
//!
//! | code | command | payload |
//! |---|---|---|
//! | 0 | end | |
//! | 1 | stroke | kind (0 stem, 1 twig), count, then count × (x, y, width, alpha) |
//! | 2 | leaf | x, y, `dir_x`, `dir_y`, size, alpha, link |
//! | 3 | flower | x, y, `dir_x`, `dir_y`, size, open, alpha, link |
//! | 4 | seed | x, y, pulse, prominence |
//!
//! `link` is the index of the developer's link this shape carries, or -1.
//!
//! On targets other than wasm32 the crate compiles to nothing, so
//! `cargo build`/`cargo test` across the whole workspace work on any machine.

#![cfg_attr(target_arch = "wasm32", no_std)]
// Security posture: every `unsafe` must carry a SAFETY justification.
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

#[cfg(target_arch = "wasm32")]
mod wasm {
    use core::arch::wasm32;
    use core::panic::PanicInfo;
    use seedling::{Flower, Leaf, Plant, Renderer, Seed, StrokeKind, StrokePoint};

    #[panic_handler]
    fn panic(_: &PanicInfo) -> ! {
        wasm32::unreachable()
    }

    const MAX_OUT: usize = 120_000; // floats in the draw list

    const CMD_END: f32 = 0.0;
    const CMD_STROKE: f32 = 1.0;
    const CMD_LEAF: f32 = 2.0;
    const CMD_FLOWER: f32 = 3.0;
    const CMD_SEED: f32 = 4.0;

    /// The draw list, filled by [`Renderer`] calls.
    struct Out {
        buf: [f32; MAX_OUT],
        len: usize,
    }

    impl Out {
        const fn put(&mut self, v: f32) {
            if self.len < MAX_OUT {
                self.buf[self.len] = v;
                self.len += 1;
            }
        }
        fn put_all(&mut self, vals: &[f32]) {
            for &v in vals {
                self.put(v);
            }
        }
    }

    impl Renderer for Out {
        fn seed(&mut self, s: Seed) {
            self.put_all(&[CMD_SEED, s.x, s.y, s.pulse, s.prominence]);
        }
        fn stroke_begin(&mut self, kind: StrokeKind, points: usize) {
            let k = if kind == StrokeKind::Stem { 0.0 } else { 1.0 };
            // A stroke has at most MAX_STEM_POINTS + 1 points (< 2^24), so
            // the count converts to f32 exactly.
            #[allow(clippy::cast_precision_loss)]
            let n = points as f32;
            self.put_all(&[CMD_STROKE, k, n]);
        }
        fn stroke_point(&mut self, p: StrokePoint) {
            self.put_all(&[p.x, p.y, p.width, p.alpha]);
        }
        fn leaf(&mut self, l: Leaf) {
            self.put_all(&[CMD_LEAF, l.x, l.y, l.dir_x, l.dir_y, l.size, l.alpha, link(l.link)]);
        }
        fn flower(&mut self, f: Flower) {
            self.put_all(&[CMD_FLOWER, f.x, f.y, f.dir_x, f.dir_y, f.size, f.open, f.alpha, link(f.link)]);
        }
    }

    /// A link index as a draw-list value: 0..=63, or -1 for none.
    fn link(l: Option<u8>) -> f32 {
        l.map_or(-1.0, f32::from)
    }

    struct State {
        plant: Plant,
        out: Out,
    }

    // All-zero initial state: the whole thing lands in .bss and adds nothing
    // to the .wasm size.
    static mut STATE: State = State { plant: Plant::blank(), out: Out { buf: [0.0; MAX_OUT], len: 0 } };

    /// Access the single simulation state.
    ///
    /// # Safety
    /// The caller must not hold any other reference into `STATE` while the
    /// returned one is alive. Each exported function below calls this exactly
    /// once and drops the reference before returning; WebAssembly here is
    /// single-threaded and the module imports nothing, so no export can
    /// re-enter another while it runs.
    #[inline]
    unsafe fn state() -> &'static mut State {
        // SAFETY: uniqueness is guaranteed by the caller contract above.
        // `addr_of_mut!` takes the address without creating an intermediate
        // reference to the `static mut`.
        unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
    }

    /// Set up a fresh plant in a `w` × `h` px viewport. `seed` makes each
    /// visit's plant different. Hostile sizes are clamped by the library.
    #[unsafe(no_mangle)]
    pub extern "C" fn init(w: f32, h: f32, seed: u32) {
        // SAFETY: the only reference to STATE during this call (see `state`).
        let s = unsafe { state() };
        s.plant.init(w, h, seed);
    }

    /// Hand out `links` links (clamped to 0..=64) to leaves and flowers, and
    /// keep the plant when the pointer leaves if `persist` is non-zero.
    /// Other settings keep their defaults. Takes effect immediately.
    #[unsafe(no_mangle)]
    pub extern "C" fn configure(links: u32, persist: i32) {
        // SAFETY: the only reference to STATE during this call (see `state`).
        let s = unsafe { state() };
        let mut c = s.plant.config();
        c.links = u8::try_from(links).unwrap_or(u8::MAX); // the library clamps to 64
        c.persist = persist != 0;
        s.plant.set_config(c);
    }

    /// Advance by `dt` seconds, then rebuild the draw list. `active` says
    /// what the pointer is doing: 0 = gone, 1 = at (`mx`, `my`), 2 = resting
    /// on a link (hold still). Returns how many floats of the draw list are
    /// valid (always ≥ 1).
    #[unsafe(no_mangle)]
    pub extern "C" fn tick(dt: f32, mx: f32, my: f32, active: i32) -> usize {
        // SAFETY: the only reference to STATE during this call (see `state`).
        let s = unsafe { state() };
        match active {
            0 => s.plant.step(dt, None),
            2 => s.plant.rest(dt),
            _ => s.plant.step(dt, Some((mx, my))),
        }
        s.out.len = 0;
        s.plant.draw(&mut s.out);
        s.out.put(CMD_END);
        s.out.len
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn out_ptr() -> *const f32 {
        // SAFETY: the only reference to STATE during this call (see `state`).
        unsafe { state() }.out.buf.as_ptr()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn ground_y() -> f32 {
        // SAFETY: the only reference to STATE during this call (see `state`).
        unsafe { state() }.plant.ground_y()
    }
}
