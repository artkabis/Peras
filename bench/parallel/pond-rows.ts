// The pond of a thousand lanterns on workers (decision 0031): pond.ts with
// the rows it draws, `top` to `bottom`, so that workers over one
// SharedArrayBuffer each draw a band, the TypeScript a user writes.

/** lowbias32 (Wellons, 2018) of a lattice point and a seed. */
function hash(x: number, y: number, seed: number): number {
  let h = (Math.imul(x, 374761393) + Math.imul(y, 668265263) + Math.imul(seed, 1274126177)) | 0;
  h ^= h >>> 16;
  h = Math.imul(h, 2146121005);
  h ^= h >>> 15;
  h = Math.imul(h, -2073254261);
  h ^= h >>> 16;
  return h;
}

/** 24 bits of a hash as a number in [0, 1). */
function unit(h: number): number {
  return (h & 16777215) / 16777216;
}

/** A triangle wave of period 1, from 1 to 0 and back: a sway without sin. */
function tri(x: number): number {
  return Math.abs(2 * (x - Math.floor(x)) - 1);
}

/** A colour component, 0 to 255. */
function byte(c: number): number {
  const v = c < 0 ? 0 : c > 1 ? 1 : c;
  return Math.floor(v * 255 + 0.5);
}

// The lantern's mesh, shared by every lantern: three rings of eight points,
// at the bottom, the middle and the top, and their triangles.
const RING = 8;
const S = Math.sqrt(0.5);
const DX = [1, S, 0, -S, -1, -S, 0, S];
const DZ = [0, S, 1, S, 0, -S, -1, -S];
const MX: number[] = [];
const MY: number[] = [];
const MZ: number[] = [];
for (let ring = 0; ring < 3; ring++) {
  const radius = ring === 1 ? 1 : 0.7;
  for (let k = 0; k < RING; k++) {
    MX.push(DX[k]! * radius);
    MY.push(ring * 0.8);
    MZ.push(DZ[k]! * radius);
  }
}
const TRIS: number[] = [];
for (let band = 0; band < 2; band++) {
  for (let k = 0; k < RING; k++) {
    const a = band * RING + k;
    const b = band * RING + ((k + 1) % RING);
    TRIS.push(a, b, a + RING, b, b + RING, a + RING);
  }
}
for (let k = 1; k < RING - 1; k++) {
  TRIS.push(0, k, k + 1);
  TRIS.push(2 * RING, 2 * RING + k + 1, 2 * RING + k);
}

const EYE = 3;
const MOON_X = 0.78;
const MOON_Y = 0.16;
const MOON_R = 0.045;
const WARM_R = [1.0, 1.0, 0.95, 1.0];
const WARM_G = [0.55, 0.35, 0.75, 0.45];
const WARM_B = [0.2, 0.25, 0.35, 0.55];

/**
 * Draws frame `frame` of `lanterns` lanterns into `pixels`, one RGBA word per
 * pixel; `depth` and `light` (three per pixel) are the caller's, of
 * `width * height` and three times that. Gives the number of pixels filled
 * by a lantern or its reflection.
 */
export function render(
  pixels: Int32Array,
  depth: Float64Array,
  light: Float64Array,
  width: number,
  height: number,
  lanterns: number,
  frame: number,
  top = 0,
  bottom = height - 1,
): number {
  const horizon = Math.floor(height * 0.42);
  const focal = width * 0.9;
  const t = frame;
  const moonX = width * MOON_X;
  const moonY = height * MOON_Y;
  const moonR = width * MOON_R;
  // The sky, its stars twinkling, and the water, darker near.
  for (let py = top; py <= bottom; py++) {
    for (let px = 0; px < width; px++) {
      const p = py * width + px;
      depth[p] = 1e9;
      let r: number;
      let g: number;
      let b: number;
      if (py < horizon) {
        const v = py / horizon;
        r = 0.02 + 0.06 * v;
        g = 0.03 + 0.08 * v;
        b = 0.1 + 0.14 * v;
        // The moon, its edge soft.
        const mx = (px + 0.5 - moonX) / moonR;
        const my = (py + 0.5 - moonY) / moonR;
        const m2 = mx * mx + my * my;
        if (m2 < 1.44) {
          const m = m2 < 1 ? 1 : (1.44 - m2) / 0.44;
          r += 0.85 * m;
          g += 0.83 * m;
          b += 0.75 * m;
        }
        if (unit(hash(px, py, 7)) < 0.0015) {
          const s =
            (0.5 + 0.5 * unit(hash(px, py, 8))) *
            (0.6 + 0.4 * tri(unit(hash(px, py, 9)) + t * 0.02));
          r += s;
          g += s;
          b += s;
        }
      } else {
        const d = (py - horizon) / (height - horizon);
        r = 0.01;
        g = 0.02 + 0.03 * d;
        b = 0.05 + 0.07 * d;
        // The moon's path on the water, broken by ripples.
        const off = Math.abs(px + 0.5 - moonX + 6 * (2 * tri(py * 0.11 + t * 0.03) - 1));
        const band = moonR * (0.6 + 2.5 * d);
        if (off < band) {
          const glint = (1 - off / band) * tri(py * 0.37 + t * 0.05) * (0.5 - 0.35 * d);
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
  const vertices = MX.length;
  const sx = new Float64Array(vertices);
  const sy = new Float64Array(vertices);
  const sz = new Float64Array(vertices);
  const wx = new Float64Array(vertices);
  const wy = new Float64Array(vertices);
  const wz = new Float64Array(vertices);
  let filled = 0;
  for (let i = 0; i < lanterns; i++) {
    // Where the lantern is, how it turns, its colour and its flame.
    let x = -45 + 90 * unit(hash(i, 1, 11)) + (unit(hash(i, 3, 11)) - 0.5) * 0.02 * t;
    x -= 90 * Math.floor((x + 45) / 90);
    let z = 120 * unit(hash(i, 2, 11)) - (0.01 + 0.03 * unit(hash(i, 4, 11))) * t;
    z = 6 + z - 120 * Math.floor(z / 120);
    const a = 2 * tri(unit(hash(i, 5, 11)) + 0.002 * t) - 1;
    const cos = (1 - a * a) / (1 + a * a);
    const sin = (2 * a) / (1 + a * a);
    const size = 0.35 + 0.25 * unit(hash(i, 7, 11));
    let lift = 0.05 * tri(unit(hash(i, 11, 11)) + 0.01 * t);
    if ((hash(i, 12, 11) & 3) === 0) {
      // A sky lantern: it rises, and starts again from the water.
      const rise = unit(hash(i, 13, 11)) + 0.0015 * t;
      lift += 18 * (rise - Math.floor(rise));
    }
    const hue = hash(i, 8, 11) & 3;
    const flame =
      0.75 + 0.25 * tri(unit(hash(i, 9, 11)) + t * (0.03 + 0.04 * unit(hash(i, 10, 11))));
    const cr = WARM_R[hue]!;
    const cg = WARM_G[hue]!;
    const cb = WARM_B[hue]!;
    for (let v = 0; v < vertices; v++) {
      const mx = MX[v]! * size;
      const mz = MZ[v]! * size;
      wx[v] = x + mx * cos - mz * sin;
      wy[v] = lift + MY[v]! * size;
      wz[v] = z + mx * sin + mz * cos;
    }
    // Twice: the lantern, then its reflection, the world mirrored in the
    // water and shaken by ripples.
    for (let mirror = 0; mirror < 2; mirror++) {
      const sign = mirror === 0 ? 1 : -1;
      for (let v = 0; v < vertices; v++) {
        const zz = wz[v]!;
        const yy = horizon - (focal * (sign * wy[v]! - EYE)) / zz;
        let xx = width / 2 + (focal * wx[v]!) / zz;
        if (mirror === 1) xx += 2.5 * (2 * tri(yy * 0.045 + t * 0.02) - 1);
        sx[v] = xx;
        sy[v] = yy;
        sz[v] = zz;
      }
      for (let k = 0; k < TRIS.length; k += 3) {
        const i0 = TRIS[k]!;
        const i1 = TRIS[k + 1]!;
        const i2 = TRIS[k + 2]!;
        const x0 = sx[i0]!;
        const y0 = sy[i0]!;
        const x1 = sx[i1]!;
        const y1 = sy[i1]!;
        const x2 = sx[i2]!;
        const y2 = sy[i2]!;
        const area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        if (area === 0) continue;
        // Flat shading: paper lit from inside, brighter where it faces the eye.
        const ax = wx[i1]! - wx[i0]!;
        const ay = wy[i1]! - wy[i0]!;
        const az = wz[i1]! - wz[i0]!;
        const bx = wx[i2]! - wx[i0]!;
        const by = wy[i2]! - wy[i0]!;
        const bz = wz[i2]! - wz[i0]!;
        const nx = ay * bz - az * by;
        const ny = az * bx - ax * bz;
        const nz = ax * by - ay * bx;
        const ex = -wx[i0]!;
        const ey = EYE - wy[i0]!;
        const ez = -wz[i0]!;
        const dot = nx * ex + ny * ey + nz * ez;
        const len = Math.sqrt((nx * nx + ny * ny + nz * nz) * (ex * ex + ey * ey + ez * ez));
        const facing = len > 0 ? Math.abs(dot) / len : 0;
        const shade = flame * (0.45 + 0.55 * facing) * (mirror === 0 ? 1 : 0.4);
        const r = cr * shade;
        const g = cg * shade;
        const b = cb * shade;
        const z0 = sz[i0]!;
        const z1 = sz[i1]!;
        const z2 = sz[i2]!;
        let minX = Math.floor(Math.min(x0, x1, x2));
        let maxX = Math.floor(Math.max(x0, x1, x2));
        let minY = Math.floor(Math.min(y0, y1, y2));
        let maxY = Math.floor(Math.max(y0, y1, y2));
        if (minX < 0) minX = 0;
        if (minY < top) minY = top;
        if (maxX > width - 1) maxX = width - 1;
        if (maxY > bottom) maxY = bottom;
        for (let py = minY; py <= maxY; py++) {
          const cy = py + 0.5;
          for (let px = minX; px <= maxX; px++) {
            const cx = px + 0.5;
            const w0 = (x2 - x1) * (cy - y1) - (y2 - y1) * (cx - x1);
            const w1 = (x0 - x2) * (cy - y2) - (y0 - y2) * (cx - x2);
            const w2 = (x1 - x0) * (cy - y0) - (y1 - y0) * (cx - x0);
            const inside = area > 0 ? w0 >= 0 && w1 >= 0 && w2 >= 0 : w0 <= 0 && w1 <= 0 && w2 <= 0;
            if (!inside) continue;
            const zz = (w0 * z0 + w1 * z1 + w2 * z2) / area;
            const p = py * width + px;
            if (zz < depth[p]!) {
              if (depth[p] === 1e9) filled++;
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
    for (let mirror = 0; mirror < 2; mirror++) {
      const sign = mirror === 0 ? 1 : -1;
      const cy = horizon - (focal * (sign * (lift + 0.8 * size) - EYE)) / z;
      let cx = width / 2 + (focal * x) / z;
      if (mirror === 1) cx += 2.5 * (2 * tri(cy * 0.045 + t * 0.02) - 1);
      const reach = (focal * 2.2 * size) / z;
      const glow = 0.45 * flame * (mirror === 0 ? 1 : 0.5);
      let minX = Math.floor(cx - reach);
      let maxX = Math.floor(cx + reach);
      let minY = Math.floor(cy - reach);
      let maxY = Math.floor(cy + reach);
      if (minX < 0) minX = 0;
      if (minY < top) minY = top;
      if (maxX > width - 1) maxX = width - 1;
      if (maxY > bottom) maxY = bottom;
      for (let py = minY; py <= maxY; py++) {
        const dy = py + 0.5 - cy;
        for (let px = minX; px <= maxX; px++) {
          const dx = px + 0.5 - cx;
          const d2 = (dx * dx + dy * dy) / (reach * reach);
          if (d2 >= 1) continue;
          const k = (1 - d2) * (1 - d2) * glow;
          const p = 3 * (py * width + px);
          light[p] = light[p]! + cr * k;
          light[p + 1] = light[p + 1]! + cg * k;
          light[p + 2] = light[p + 2]! + cb * k;
        }
      }
    }
  }
  for (let p = top * width; p < (bottom + 1) * width; p++) {
    pixels[p] =
      byte(light[3 * p]!) +
      byte(light[3 * p + 1]!) * 256 +
      byte(light[3 * p + 2]!) * 65536 -
      16777216;
  }
  return filled;
}
