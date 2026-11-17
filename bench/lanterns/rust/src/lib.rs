//! The pond of a thousand lanterns (decision 0029), written as a Rust
//! developer writes it, from the specification of `bench/lanterns/pond.ts`:
//! it must give the same bytes, pixel for pixel. The buffers stay in the
//! module's memory, the pixels read by JavaScript at the address `ptr` gives,
//! with no copy and no `unsafe`.

// The names of the specification, as a formula writes them.
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use wasm_bindgen::prelude::*;

/// A frame's buffers, kept between frames: the pixels, one RGBA word each,
/// the depth of each, and its light, three per pixel.
#[wasm_bindgen]
pub struct Frame {
    pixels: Vec<u32>,
    depth: Vec<f64>,
    light: Vec<f64>,
}

#[wasm_bindgen]
impl Frame {
    /// The buffers of `n` pixels.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self {
            pixels: vec![0; n],
            depth: vec![0.0; n],
            light: vec![0.0; 3 * n],
        }
    }

    /// Where the pixels start, for a `Uint32Array` over the memory.
    #[must_use]
    pub fn ptr(&self) -> usize {
        self.pixels.as_ptr() as usize
    }
}

/// lowbias32 (Wellons, 2018) of a lattice point and a seed.
fn hash(x: i32, y: i32, seed: i32) -> i32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(1_274_126_177))
        .cast_unsigned();
    h ^= h >> 16;
    h = h.wrapping_mul(2_146_121_005);
    h ^= h >> 15;
    h = h.wrapping_mul(2_221_713_035);
    h ^= h >> 16;
    h.cast_signed()
}

/// 24 bits of a hash as a number in [0, 1).
fn unit(h: i32) -> f64 {
    f64::from(h & 16_777_215) / 16_777_216.0
}

/// A triangle wave of period 1, from 1 to 0 and back: a sway without sin.
fn tri(x: f64) -> f64 {
    (2.0 * (x - x.floor()) - 1.0).abs()
}

/// A colour component, 0 to 255.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(c: f64) -> u32 {
    (c.clamp(0.0, 1.0) * 255.0 + 0.5).floor() as u32
}

/// The lantern's mesh, shared by every lantern: three rings of eight points,
/// at the bottom, the middle and the top, and their triangles.
struct Mesh {
    x: Vec<f64>,
    y: Vec<f64>,
    z: Vec<f64>,
    tris: Vec<usize>,
}

fn lantern() -> Mesh {
    let s = 0.5_f64.sqrt();
    let dx = [1.0, s, 0.0, -s, -1.0, -s, 0.0, s];
    let dz = [0.0, s, 1.0, s, 0.0, -s, -1.0, -s];
    let mut mesh = Mesh {
        x: Vec::new(),
        y: Vec::new(),
        z: Vec::new(),
        tris: Vec::new(),
    };
    for ring in 0..3 {
        let radius = if ring == 1 { 1.0 } else { 0.7 };
        for k in 0..8 {
            mesh.x.push(dx[k] * radius);
            mesh.y.push(f64::from(ring) * 0.8);
            mesh.z.push(dz[k] * radius);
        }
    }
    for band in 0..2 {
        for k in 0..8 {
            let a = band * 8 + k;
            let b = band * 8 + (k + 1) % 8;
            mesh.tris.extend([a, b, a + 8, b, b + 8, a + 8]);
        }
    }
    for k in 1..7 {
        mesh.tris.extend([0, k, k + 1, 16, 16 + k + 1, 16 + k]);
    }
    mesh
}

const EYE: f64 = 3.0;
const WARM: [[f64; 3]; 4] = [
    [1.0, 0.55, 0.2],
    [1.0, 0.35, 0.25],
    [0.95, 0.75, 0.35],
    [1.0, 0.45, 0.55],
];

/// The first and last pixel of a span, kept on the screen.
#[allow(clippy::cast_possible_truncation)]
fn span(lo: f64, hi: f64, size: i32) -> (i32, i32) {
    (
        (lo.floor() as i32).max(0),
        (hi.floor() as i32).min(size - 1),
    )
}

/// Draws frame `frame` of `lanterns` lanterns; gives the number of pixels
/// filled by a lantern or its reflection.
#[wasm_bindgen]
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::needless_range_loop
)]
pub fn render(buffers: &mut Frame, width: i32, height: i32, lanterns: i32, frame: i32) -> u32 {
    let Frame {
        pixels,
        depth,
        light,
    } = buffers;
    let horizon = (f64::from(height) * 0.42).floor() as i32;
    let w = f64::from(width);
    let focal = w * 0.9;
    let t = f64::from(frame);
    let (moon_x, moon_y, moon_r) = (w * 0.78, f64::from(height) * 0.16, w * 0.045);
    let row = width as usize;
    // The sky, its stars twinkling, and the water, darker near.
    for py in 0..height {
        for px in 0..width {
            let p = py as usize * row + px as usize;
            depth[p] = 1e9;
            let (mut r, mut g, mut b);
            if py < horizon {
                let v = f64::from(py) / f64::from(horizon);
                r = 0.02 + 0.06 * v;
                g = 0.03 + 0.08 * v;
                b = 0.1 + 0.14 * v;
                // The moon, its edge soft.
                let mx = (f64::from(px) + 0.5 - moon_x) / moon_r;
                let my = (f64::from(py) + 0.5 - moon_y) / moon_r;
                let m2 = mx * mx + my * my;
                if m2 < 1.44 {
                    let m = if m2 < 1.0 { 1.0 } else { (1.44 - m2) / 0.44 };
                    r += 0.85 * m;
                    g += 0.83 * m;
                    b += 0.75 * m;
                }
                if unit(hash(px, py, 7)) < 0.0015 {
                    let s = (0.5 + 0.5 * unit(hash(px, py, 8)))
                        * (0.6 + 0.4 * tri(unit(hash(px, py, 9)) + t * 0.02));
                    r += s;
                    g += s;
                    b += s;
                }
            } else {
                let d = f64::from(py - horizon) / f64::from(height - horizon);
                r = 0.01;
                g = 0.02 + 0.03 * d;
                b = 0.05 + 0.07 * d;
                // The moon's path on the water, broken by ripples.
                let off = (f64::from(px) + 0.5 - moon_x
                    + 6.0 * (2.0 * tri(f64::from(py) * 0.11 + t * 0.03) - 1.0))
                    .abs();
                let band = moon_r * (0.6 + 2.5 * d);
                if off < band {
                    let glint = (1.0 - off / band)
                        * tri(f64::from(py) * 0.37 + t * 0.05)
                        * (0.5 - 0.35 * d);
                    r += 0.8 * glint;
                    g += 0.78 * glint;
                    b += 0.7 * glint;
                }
            }
            light[3 * p] = r;
            light[3 * p + 1] = g;
            light[3 * p + 2] = b;
        }
    }
    let mesh = lantern();
    let vertices = mesh.x.len();
    let (mut sx, mut sy, mut sz) = (
        vec![0.0; vertices],
        vec![0.0; vertices],
        vec![0.0; vertices],
    );
    let (mut wx, mut wy, mut wz) = (
        vec![0.0; vertices],
        vec![0.0; vertices],
        vec![0.0; vertices],
    );
    let mut filled = 0;
    for i in 0..lanterns {
        // Where the lantern is, how it turns, its colour and its flame.
        let mut x = -45.0 + 90.0 * unit(hash(i, 1, 11)) + (unit(hash(i, 3, 11)) - 0.5) * 0.02 * t;
        x -= 90.0 * ((x + 45.0) / 90.0).floor();
        let mut z = 120.0 * unit(hash(i, 2, 11)) - (0.01 + 0.03 * unit(hash(i, 4, 11))) * t;
        z = 6.0 + z - 120.0 * (z / 120.0).floor();
        let a = 2.0 * tri(unit(hash(i, 5, 11)) + 0.002 * t) - 1.0;
        let cos = (1.0 - a * a) / (1.0 + a * a);
        let sin = (2.0 * a) / (1.0 + a * a);
        let size = 0.35 + 0.25 * unit(hash(i, 7, 11));
        let mut lift = 0.05 * tri(unit(hash(i, 11, 11)) + 0.01 * t);
        if hash(i, 12, 11).trailing_zeros() >= 2 {
            // A sky lantern: it rises, and starts again from the water.
            let rise = unit(hash(i, 13, 11)) + 0.0015 * t;
            lift += 18.0 * (rise - rise.floor());
        }
        let [cr, cg, cb] = WARM[(hash(i, 8, 11) & 3) as usize];
        let flame =
            0.75 + 0.25 * tri(unit(hash(i, 9, 11)) + t * (0.03 + 0.04 * unit(hash(i, 10, 11))));
        for v in 0..vertices {
            let mx = mesh.x[v] * size;
            let mz = mesh.z[v] * size;
            wx[v] = x + mx * cos - mz * sin;
            wy[v] = lift + mesh.y[v] * size;
            wz[v] = z + mx * sin + mz * cos;
        }
        // Twice: the lantern, then its reflection, the world mirrored in the
        // water and shaken by ripples.
        for mirror in [false, true] {
            let sign = if mirror { -1.0 } else { 1.0 };
            for v in 0..vertices {
                let zz = wz[v];
                let yy = f64::from(horizon) - (focal * (sign * wy[v] - EYE)) / zz;
                let mut xx = w / 2.0 + (focal * wx[v]) / zz;
                if mirror {
                    xx += 2.5 * (2.0 * tri(yy * 0.045 + t * 0.02) - 1.0);
                }
                sx[v] = xx;
                sy[v] = yy;
                sz[v] = zz;
            }
            for &[i0, i1, i2] in mesh.tris.as_chunks::<3>().0 {
                let (x0, y0, x1, y1, x2, y2) = (sx[i0], sy[i0], sx[i1], sy[i1], sx[i2], sy[i2]);
                let area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
                if area == 0.0 {
                    continue;
                }
                // Flat shading: paper lit from inside, brighter where it faces the eye.
                let (ax, ay, az) = (wx[i1] - wx[i0], wy[i1] - wy[i0], wz[i1] - wz[i0]);
                let (bx, by, bz) = (wx[i2] - wx[i0], wy[i2] - wy[i0], wz[i2] - wz[i0]);
                let (nx, ny, nz) = (ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx);
                let (ex, ey, ez) = (-wx[i0], EYE - wy[i0], -wz[i0]);
                let dot = nx * ex + ny * ey + nz * ez;
                let len = ((nx * nx + ny * ny + nz * nz) * (ex * ex + ey * ey + ez * ez)).sqrt();
                let facing = if len > 0.0 { dot.abs() / len } else { 0.0 };
                let shade = flame * (0.45 + 0.55 * facing) * if mirror { 0.4 } else { 1.0 };
                let (r, g, b) = (cr * shade, cg * shade, cb * shade);
                let (z0, z1, z2) = (sz[i0], sz[i1], sz[i2]);
                let (min_x, max_x) = span(x0.min(x1).min(x2), x0.max(x1).max(x2), width);
                let (min_y, max_y) = span(y0.min(y1).min(y2), y0.max(y1).max(y2), height);
                for py in min_y..=max_y {
                    let cy = f64::from(py) + 0.5;
                    for px in min_x..=max_x {
                        let cx = f64::from(px) + 0.5;
                        let w0 = (x2 - x1) * (cy - y1) - (y2 - y1) * (cx - x1);
                        let w1 = (x0 - x2) * (cy - y2) - (y0 - y2) * (cx - x2);
                        let w2 = (x1 - x0) * (cy - y0) - (y1 - y0) * (cx - x0);
                        let inside = if area > 0.0 {
                            w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
                        } else {
                            w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
                        };
                        if !inside {
                            continue;
                        }
                        let zz = (w0 * z0 + w1 * z1 + w2 * z2) / area;
                        let p = py as usize * row + px as usize;
                        if zz < depth[p] {
                            #[allow(clippy::float_cmp)]
                            if depth[p] == 1e9 {
                                filled += 1;
                            }
                            depth[p] = zz;
                            light[3 * p] = r;
                            light[3 * p + 1] = g;
                            light[3 * p + 2] = b;
                        }
                    }
                }
            }
        }
        // The flame's halo, and its reflection.
        for mirror in [false, true] {
            let sign = if mirror { -1.0 } else { 1.0 };
            let cy = f64::from(horizon) - (focal * (sign * (lift + 0.8 * size) - EYE)) / z;
            let mut cx = w / 2.0 + (focal * x) / z;
            if mirror {
                cx += 2.5 * (2.0 * tri(cy * 0.045 + t * 0.02) - 1.0);
            }
            let reach = (focal * 2.2 * size) / z;
            let glow = 0.45 * flame * if mirror { 0.5 } else { 1.0 };
            let (min_x, max_x) = span(cx - reach, cx + reach, width);
            let (min_y, max_y) = span(cy - reach, cy + reach, height);
            for py in min_y..=max_y {
                let dy = f64::from(py) + 0.5 - cy;
                for px in min_x..=max_x {
                    let dx = f64::from(px) + 0.5 - cx;
                    let d2 = (dx * dx + dy * dy) / (reach * reach);
                    if d2 >= 1.0 {
                        continue;
                    }
                    let k = (1.0 - d2) * (1.0 - d2) * glow;
                    let p = 3 * (py as usize * row + px as usize);
                    light[p] += cr * k;
                    light[p + 1] += cg * k;
                    light[p + 2] += cb * k;
                }
            }
        }
    }
    for p in 0..(row * height as usize) {
        pixels[p] = byte(light[3 * p])
            | (byte(light[3 * p + 1]) << 8)
            | (byte(light[3 * p + 2]) << 16)
            | 0xff00_0000;
    }
    filled
}
