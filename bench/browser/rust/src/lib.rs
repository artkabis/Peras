//! The kernels of `bench/browser/kernels.peras`, written as a Rust developer
//! writes them: a structure per body, `split`, `filter` and `count`, the
//! standard library's sort. Arrays of numbers stay in the module's memory, as
//! a `PerasArray` does: JavaScript reads them at the address `ptr` gives,
//! with no copy and no `unsafe`.

use wasm_bindgen::prelude::*;

/// An array of `f64` in the module's memory.
#[wasm_bindgen]
pub struct F64s(Vec<f64>);

#[wasm_bindgen]
impl F64s {
    /// `n` zeros.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self(vec![0.0; n])
    }

    /// Where the elements start, for a `Float64Array` over the memory.
    #[must_use]
    pub fn ptr(&self) -> usize {
        self.0.as_ptr() as usize
    }
}

/// An array of `i32` in the module's memory.
#[wasm_bindgen]
pub struct I32s(Vec<i32>);

#[wasm_bindgen]
impl I32s {
    /// `n` zeros.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self(vec![0; n])
    }

    /// Where the elements start, for an `Int32Array` over the memory.
    #[must_use]
    pub fn ptr(&self) -> usize {
        self.0.as_ptr() as usize
    }
}

/// Particles moving in a box, bouncing off its walls.
#[wasm_bindgen]
pub fn particles(xs: &mut F64s, vs: &mut F64s, steps: i32) -> f64 {
    let dt = 0.01;
    for _ in 0..steps {
        for (x, v) in xs.0.iter_mut().zip(vs.0.iter_mut()) {
            let mut next = *x + *v * dt;
            if !(0.0..=1.0).contains(&next) {
                *v = -*v;
                next = *x + *v * dt;
            }
            *x = next;
        }
    }
    xs.0.iter().sum()
}

/// Pixels packed as 0xAARRGGBB turned to grey in place.
#[wasm_bindgen]
pub fn grayscale(pixels: &mut I32s) -> i32 {
    let mut checksum = 0;
    for pixel in &mut pixels.0 {
        let red = (*pixel >> 16) & 255;
        let green = (*pixel >> 8) & 255;
        let blue = *pixel & 255;
        let grey = (red * 77 + green * 150 + blue * 29) >> 8;
        let out = (*pixel & -16_777_216) | (grey << 16) | (grey << 8) | grey;
        *pixel = out;
        checksum ^= out;
    }
    checksum
}

struct Body {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    m: f64,
}

/// Gravity between n bodies, each pair at each step.
#[wasm_bindgen]
#[must_use]
pub fn nbody(n: i32, steps: i32) -> f64 {
    let mut bodies: Vec<Body> = (0..n)
        .map(|i| Body {
            x: f64::from(i * 7 % 100),
            y: f64::from(i * 13 % 100),
            vx: 0.0,
            vy: 0.0,
            m: 1.0 + f64::from(i % 3),
        })
        .collect();
    for _ in 0..steps {
        for i in 0..bodies.len() {
            let (xi, yi) = (bodies[i].x, bodies[i].y);
            let (mut ax, mut ay) = (0.0, 0.0);
            for (j, b) in bodies.iter().enumerate() {
                if i != j {
                    let dx = b.x - xi;
                    let dy = b.y - yi;
                    let d2 = dx * dx + dy * dy + 0.01;
                    let inv = b.m / (d2 * d2.sqrt());
                    ax += dx * inv;
                    ay += dy * inv;
                }
            }
            bodies[i].vx += ax * 0.001;
            bodies[i].vy += ay * 0.001;
        }
        for b in &mut bodies {
            b.x += b.vx * 0.001;
            b.y += b.vy * 0.001;
        }
    }
    bodies
        .iter()
        .map(|b| b.m * (b.vx * b.vx + b.vy * b.vy))
        .sum()
}

/// FNV-1a over n integers, on 64 bits.
#[wasm_bindgen]
#[must_use]
pub fn hash64(n: i32) -> i64 {
    let mut h: i64 = -3_750_763_034_362_895_579;
    for i in 0..n {
        h = (h ^ i64::from(i)).wrapping_mul(1_099_511_628_211);
    }
    h
}

/// Sorting numbers in place, in the order of JavaScript's typed arrays.
#[wasm_bindgen]
pub fn sorted(xs: &mut F64s) -> f64 {
    xs.0.sort_unstable_by(f64::total_cmp);
    let n = xs.0.len();
    xs.0[0] + xs.0[n / 2] + xs.0[n - 1]
}

/// Points inside the Mandelbrot set.
#[wasm_bindgen]
#[must_use]
pub fn mandel(size: i32) -> i32 {
    let mut inside = 0;
    let scale = 3.0 / f64::from(size);
    for py in 0..size {
        for px in 0..size {
            let cx = f64::from(px) * scale - 2.0;
            let cy = f64::from(py) * scale - 1.5;
            let (mut x, mut y) = (0.0_f64, 0.0_f64);
            let mut step = 0;
            while step < 64 && x * x + y * y <= 4.0 {
                let next = x * x - y * y + cx;
                y = 2.0 * x * y + cy;
                x = next;
                step += 1;
            }
            if step == 64 {
                inside += 1;
            }
        }
    }
    inside
}

/// Words of a text: split, filter, count.
#[wasm_bindgen]
#[must_use]
pub fn words(text: &str) -> i32 {
    i32::try_from(text.split(' ').filter(|w| !w.is_empty()).count()).unwrap_or(i32::MAX)
}

/// The smallest function, to measure what a call from JavaScript costs.
#[wasm_bindgen]
#[must_use]
pub fn add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}
