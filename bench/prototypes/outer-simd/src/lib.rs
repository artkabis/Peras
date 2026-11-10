//! Mandelbrot and n-body, each twice: as Peras compiles them today, one
//! value at a time, and vectorized across their outer loop, two `f64` lanes
//! at a time with WebAssembly SIMD. Vectorizing across the outer loop keeps
//! every lane's arithmetic, in the same order: the results are the same, bit
//! for bit, which `run.mjs` checks. The arrays are columns, as Peras stores
//! an array of structures. No `unsafe`: the SIMD functions used are safe.

use wasm_bindgen::prelude::wasm_bindgen;

use core::arch::wasm32::{
    f64x2, f64x2_add, f64x2_div, f64x2_extract_lane, f64x2_le, f64x2_mul, f64x2_splat, f64x2_sqrt,
    f64x2_sub, i64x2, i64x2_eq, i64x2_extract_lane, i64x2_splat, i64x2_sub, v128, v128_any_true,
    v128_bitselect, v128_not,
};

/// Points inside the Mandelbrot set, one at a time: the kernel as written.
#[wasm_bindgen]
#[must_use]
pub fn mandel_scalar(size: i32) -> i32 {
    let mut inside = 0;
    let scale = 3.0 / f64::from(size);
    for py in 0..size {
        for px in 0..size {
            if escape(f64::from(px) * scale - 2.0, f64::from(py) * scale - 1.5) == 64 {
                inside += 1;
            }
        }
    }
    inside
}

/// The steps before (cx, cy) escapes, at most 64.
fn escape(cx: f64, cy: f64) -> i32 {
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    let mut step = 0;
    while step < 64 && x * x + y * y <= 4.0 {
        let next = x * x - y * y + cx;
        y = 2.0 * x * y + cy;
        x = next;
        step += 1;
    }
    step
}

/// Points inside the Mandelbrot set, two neighbours at a time: each lane runs
/// while it has not escaped, a lane that has keeps its values.
#[wasm_bindgen]
#[must_use]
pub fn mandel_simd(size: i32) -> i32 {
    let mut inside = 0;
    let scale = 3.0 / f64::from(size);
    let (four, two, max) = (f64x2_splat(4.0), f64x2_splat(2.0), i64x2_splat(64));
    for py in 0..size {
        let cy = f64x2_splat(f64::from(py) * scale - 1.5);
        let mut px = 0;
        while px + 1 < size {
            let cx = f64x2(f64::from(px) * scale - 2.0, f64::from(px + 1) * scale - 2.0);
            let (mut x, mut y) = (f64x2_splat(0.0), f64x2_splat(0.0));
            let mut steps = i64x2_splat(0);
            // One counter for both lanes: a lane stops counting when it escapes.
            for _ in 0..64 {
                let (xx, yy) = (f64x2_mul(x, x), f64x2_mul(y, y));
                let running = f64x2_le(f64x2_add(xx, yy), four);
                if !v128_any_true(running) {
                    break;
                }
                let next = f64x2_add(f64x2_sub(xx, yy), cx);
                let next_y = f64x2_add(f64x2_mul(f64x2_mul(two, x), y), cy);
                x = v128_bitselect(next, x, running);
                y = v128_bitselect(next_y, y, running);
                // `running` is -1 in a running lane: subtracting it counts.
                steps = i64x2_sub(steps, running);
            }
            let done = i64x2_eq(steps, max);
            inside -= i32::try_from(i64x2_extract_lane::<0>(done) + i64x2_extract_lane::<1>(done))
                .unwrap_or(0);
            px += 2;
        }
        if px < size && escape(f64::from(px) * scale - 2.0, f64::from(py) * scale - 1.5) == 64 {
            inside += 1;
        }
    }
    inside
}

/// An index as a lane's value: on wasm32, `usize` has 32 bits, so this never
/// fails.
fn lane(i: usize) -> i64 {
    i64::try_from(i).unwrap_or(-1)
}

/// Bodies in columns, as Peras stores `Array<Body>`.
struct Bodies {
    x: Vec<f64>,
    y: Vec<f64>,
    vx: Vec<f64>,
    vy: Vec<f64>,
    m: Vec<f64>,
}

impl Bodies {
    fn new(n: i32) -> Self {
        let len = usize::try_from(n).unwrap_or(0);
        let mut b = Self {
            x: vec![0.0; len],
            y: vec![0.0; len],
            vx: vec![0.0; len],
            vy: vec![0.0; len],
            m: vec![0.0; len],
        };
        for i in 0..n {
            let k = usize::try_from(i).unwrap_or(0);
            b.x[k] = f64::from(i * 7 % 100);
            b.y[k] = f64::from(i * 13 % 100);
            b.m[k] = 1.0 + f64::from(i % 3);
        }
        b
    }

    fn moved(&mut self) {
        for i in 0..self.x.len() {
            self.x[i] += self.vx[i] * 0.001;
            self.y[i] += self.vy[i] * 0.001;
        }
    }

    fn energy(&self) -> f64 {
        let mut energy = 0.0;
        for i in 0..self.x.len() {
            energy += self.m[i] * (self.vx[i] * self.vx[i] + self.vy[i] * self.vy[i]);
        }
        energy
    }

    /// The pull on body i, one pair at a time, in order.
    fn pull(&self, i: usize) -> (f64, f64) {
        let (xi, yi) = (self.x[i], self.y[i]);
        let (mut ax, mut ay) = (0.0, 0.0);
        for j in 0..self.x.len() {
            if i != j {
                let dx = self.x[j] - xi;
                let dy = self.y[j] - yi;
                let d2 = dx * dx + dy * dy + 0.01;
                let inv = self.m[j] / (d2 * d2.sqrt());
                ax += dx * inv;
                ay += dy * inv;
            }
        }
        (ax, ay)
    }
}

/// n-body, one body at a time: the kernel as written, in columns.
#[wasm_bindgen]
#[must_use]
pub fn nbody_scalar(n: i32, steps: i32) -> f64 {
    let mut b = Bodies::new(n);
    for _ in 0..steps {
        for i in 0..b.x.len() {
            let (ax, ay) = b.pull(i);
            b.vx[i] += ax * 0.001;
            b.vy[i] += ay * 0.001;
        }
        b.moved();
    }
    b.energy()
}

/// n-body, two bodies at a time: each lane adds its own pulls in the same
/// order as one at a time; the pair j == i adds nothing to its lane.
#[wasm_bindgen]
#[must_use]
pub fn nbody_simd(n: i32, steps: i32) -> f64 {
    let mut b = Bodies::new(n);
    let len = b.x.len();
    let soft = f64x2_splat(0.01);
    for _ in 0..steps {
        let mut i = 0;
        while i + 1 < len {
            let xi = f64x2(b.x[i], b.x[i + 1]);
            let yi = f64x2(b.y[i], b.y[i + 1]);
            let (mut ax, mut ay) = (f64x2_splat(0.0), f64x2_splat(0.0));
            let lanes = i64x2(lane(i), lane(i + 1));
            for j in 0..len {
                let dx = f64x2_sub(f64x2_splat(b.x[j]), xi);
                let dy = f64x2_sub(f64x2_splat(b.y[j]), yi);
                let d2 = f64x2_add(f64x2_add(f64x2_mul(dx, dx), f64x2_mul(dy, dy)), soft);
                let inv = f64x2_div(f64x2_splat(b.m[j]), f64x2_mul(d2, f64x2_sqrt(d2)));
                // Every lane but the body itself takes the pull.
                let other: v128 = v128_not(i64x2_eq(lanes, i64x2_splat(lane(j))));
                ax = v128_bitselect(f64x2_add(ax, f64x2_mul(dx, inv)), ax, other);
                ay = v128_bitselect(f64x2_add(ay, f64x2_mul(dy, inv)), ay, other);
            }
            b.vx[i] += f64x2_extract_lane::<0>(ax) * 0.001;
            b.vy[i] += f64x2_extract_lane::<0>(ay) * 0.001;
            b.vx[i + 1] += f64x2_extract_lane::<1>(ax) * 0.001;
            b.vy[i + 1] += f64x2_extract_lane::<1>(ay) * 0.001;
            i += 2;
        }
        if i < len {
            let (ax, ay) = b.pull(i);
            b.vx[i] += ax * 0.001;
            b.vy[i] += ay * 0.001;
        }
        b.moved();
    }
    b.energy()
}
