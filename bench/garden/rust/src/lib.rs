//! The garden of Apeiron (decision 0013), written as a Rust developer writes
//! it, from the specification of `bench/garden/garden.ts`: it must give the
//! same bytes, pixel for pixel. The pixels stay in the module's memory, read
//! by JavaScript at the address `ptr` gives, with no copy and no `unsafe`.

// The names of the specification, as a formula writes them: x, y, a, b, c, d.
#![allow(clippy::many_single_char_names)]

use wasm_bindgen::prelude::*;

/// The pixels of a window, one RGBA word each.
#[wasm_bindgen]
pub struct Pixels(Vec<u32>);

#[wasm_bindgen]
impl Pixels {
    /// `n` black pixels.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self(vec![0; n])
    }

    /// Where the pixels start, for a `Uint32Array` over the memory.
    #[must_use]
    pub fn ptr(&self) -> usize {
        self.0.as_ptr() as usize
    }
}

/// lowbias32 (Wellons, 2018) of a lattice point and a seed: the bits of the
/// sum, read as unsigned.
#[allow(clippy::cast_sign_loss)]
fn hash(x: i32, y: i32, seed: i32) -> u32 {
    let mut h = (x.wrapping_mul(374_761_393))
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(1_274_126_177)) as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h
}

/// 24 bits of a hash as a number in [0, 1).
fn unit(h: u32) -> f64 {
    f64::from(h & 0x00ff_ffff) / 16_777_216.0
}

/// Value noise: the hashes of the four corners, smoothly interpolated.
fn noise(x: f64, y: f64, seed: i32) -> f64 {
    let fx = x.floor();
    let fy = y.floor();
    #[allow(clippy::cast_possible_truncation)]
    let (ix, iy) = (fx as i32, fy as i32);
    let tx = x - fx;
    let ty = y - fy;
    let ux = tx * tx * (3.0 - 2.0 * tx);
    let uy = ty * ty * (3.0 - 2.0 * ty);
    let a = unit(hash(ix, iy, seed));
    let b = unit(hash(ix + 1, iy, seed));
    let c = unit(hash(ix, iy + 1, seed));
    let d = unit(hash(ix + 1, iy + 1, seed));
    a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy
}

/// A fractal sum of noises, each twice the frequency and half the amplitude.
fn fbm(x: f64, y: f64, seed: i32, octaves: i32) -> f64 {
    let (mut sum, mut amp, mut f) = (0.0, 0.5, 1.0);
    for k in 0..octaves {
        sum += amp * noise(x * f, y * f, seed + k);
        f *= 2.0;
        amp *= 0.5;
    }
    sum
}

/// A ridge: folded noise along one axis, sharp at its crests.
fn ridge(x: f64, seed: i32, octaves: i32) -> f64 {
    let (mut sum, mut amp, mut f) = (0.0, 0.5, 1.0);
    for k in 0..octaves {
        let n = noise(x * f, 0.5, seed + k);
        let r = 1.0 - (2.0 * n - 1.0).abs();
        sum += amp * r * r;
        f *= 2.0;
        amp *= 0.5;
    }
    sum
}

fn mix(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn clamp(c: f64) -> f64 {
    // NaN stays NaN, as with the comparisons of the specification.
    c.clamp(0.0, 1.0)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(c: f64) -> u32 {
    (clamp(c) * 255.0 + 0.5).floor() as u32
}

/// The six petals around a heart: their directions.
const PETALS: [(f64, f64); 6] = [
    (1.0, 0.0),
    (0.5, 0.866_025_403_784_438_6),
    (-0.5, 0.866_025_403_784_438_6),
    (-1.0, 0.0),
    (-0.5, -0.866_025_403_784_438_6),
    (0.5, -0.866_025_403_784_438_6),
];

/// The colours of the petals, by the hue a flower draws.
const HUES: [[f64; 3]; 4] = [
    [0.95, 0.35, 0.45],
    [0.9, 0.9, 0.95],
    [0.98, 0.6, 0.2],
    [0.62, 0.45, 0.9],
];

/// The colour of a point.
fn point(x: f64, y: f64, octaves: i32, levels: i32) -> [f64; 3] {
    // The sky: zenith to horizon, and clouds where the noise rises.
    let t = clamp(y / 0.7);
    let mut c = [mix(0.2, 0.98, t), mix(0.36, 0.8, t), mix(0.7, 0.62, t)];
    let cloud = clamp((fbm(x * 4.0, y * 10.0, 100, octaves) - 0.56) * 3.0) * (1.0 - t);
    c = [
        mix(c[0], 0.97, cloud * 0.85),
        mix(c[1], 0.97, cloud * 0.85),
        mix(c[2], 1.0, cloud * 0.85),
    ];
    // The ranges, far to near: paler with distance, as the air makes them.
    for k in 0..3 {
        let kf = f64::from(k);
        let top =
            0.55 + 0.07 * kf - (0.25 - 0.05 * kf) * ridge(x * (3.0 + 3.0 * kf), 200 + k, octaves);
        if y >= top {
            let shade = 0.85 + 0.3 * fbm(x * 40.0, y * 40.0, 210 + k, octaves);
            let fade = 0.5 - 0.21 * kf;
            c = [
                mix((0.55 - 0.175 * kf) * shade, 0.98, fade),
                mix((0.52 - 0.11 * kf) * shade, 0.8, fade),
                mix((0.72 - 0.21 * kf) * shade, 0.62, fade),
            ];
        }
    }
    // The meadow, and its flowers, smaller at each level.
    if y >= 0.74 + 0.02 * noise(x * 8.0, 0.5, 350) {
        let shade = 0.75 + 0.5 * fbm(x * 60.0, y * 60.0, 300, octaves);
        c = [0.25 * shade, 0.45 * shade, 0.18 * shade];
        let mut size = 0.012;
        for level in 0..levels {
            #[allow(clippy::cast_possible_truncation)]
            let (fx, fy) = ((x / size).floor() as i32, (y / size).floor() as i32);
            for j in -1..=1 {
                for i in -1..=1 {
                    let (cx, cy) = (fx + i, fy + j);
                    if unit(hash(cx, cy, 400 + level)) >= 0.55 {
                        continue;
                    }
                    let px = (f64::from(cx) + unit(hash(cx, cy, 500 + level))) * size;
                    let py = (f64::from(cy) + unit(hash(cx, cy, 600 + level))) * size;
                    let radius = size * (0.18 + 0.12 * unit(hash(cx, cy, 700 + level)));
                    let (dx, dy) = (x - px, y - py);
                    let heart = radius * 0.3;
                    if dx * dx + dy * dy < heart * heart {
                        c = [0.98, 0.82, 0.2];
                        continue;
                    }
                    let petal = radius * 0.45;
                    let reach = radius * 0.55;
                    let hue = HUES[(hash(cx, cy, 800 + level) & 3) as usize];
                    for (ox, oy) in PETALS {
                        let (qx, qy) = (dx - ox * reach, dy - oy * reach);
                        if qx * qx + qy * qy < petal * petal {
                            c = hue;
                        }
                    }
                }
            }
            size *= 0.5;
        }
    }
    c
}

/// Draws the window of `width` × `height` pixels zoomed `zoom` times around
/// (cx, cy); gives the number of pixels drawn. Each doubling of the zoom adds
/// an octave and a level of flowers.
#[wasm_bindgen]
#[must_use]
pub fn render(pixels: &mut Pixels, width: u32, height: u32, zoom: f64, cx: f64, cy: f64) -> u32 {
    let mut level = 0;
    let mut z = zoom;
    while z >= 2.0 {
        z /= 2.0;
        level += 1;
    }
    let (octaves, levels) = (6 + level, 1 + level);
    let (w, h) = (f64::from(width), f64::from(height));
    for (row, line) in pixels
        .0
        .chunks_mut(width as usize)
        .take(height as usize)
        .enumerate()
    {
        #[allow(clippy::cast_precision_loss)]
        let y = cy + ((row as f64 + 0.5) / h - 0.5) / zoom;
        for (col, pixel) in line.iter_mut().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let x = cx + ((col as f64 + 0.5) / w - 0.5) / zoom;
            let [r, g, b] = point(x, y, octaves, levels);
            *pixel = byte(r) | (byte(g) << 8) | (byte(b) << 16) | 0xff00_0000;
        }
    }
    width * height
}
