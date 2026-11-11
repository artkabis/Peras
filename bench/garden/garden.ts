// The garden of Apeiron (decision 0013), in TypeScript written for speed:
// the reference the other versions are checked against, pixel for pixel.
//
// The plane is the square [0, 1] × [0, 1], y downward: a sky, three ranges of
// mountains, a meadow of flowers. A window of it, zoomed 2^level times around
// (cx, cy), is drawn into `pixels`, one RGBA word per pixel. Each doubling of
// the zoom adds an octave to every noise and a smaller kind of flower: the
// detail has no end. Only + - * / floor and an integer hash are used, which
// IEEE 754 and every language compute the same, so every version gives the
// same bytes.

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

/** Value noise: the hashes of the four corners, smoothly interpolated. */
function noise(x: number, y: number, seed: number): number {
  const fx = Math.floor(x);
  const fy = Math.floor(y);
  const ix = fx | 0;
  const iy = fy | 0;
  const tx = x - fx;
  const ty = y - fy;
  const ux = tx * tx * (3 - 2 * tx);
  const uy = ty * ty * (3 - 2 * ty);
  const a = unit(hash(ix, iy, seed));
  const b = unit(hash(ix + 1, iy, seed));
  const c = unit(hash(ix, iy + 1, seed));
  const d = unit(hash(ix + 1, iy + 1, seed));
  return a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy;
}

/** A fractal sum of `octaves` noises, each twice the frequency and half the amplitude. */
function fbm(x: number, y: number, seed: number, octaves: number): number {
  let sum = 0;
  let amp = 0.5;
  let f = 1;
  for (let k = 0; k < octaves; k++) {
    sum += amp * noise(x * f, y * f, seed + k);
    f *= 2;
    amp *= 0.5;
  }
  return sum;
}

/** A ridge: folded noise along one axis, sharp at its crests. */
function ridge(x: number, seed: number, octaves: number): number {
  let sum = 0;
  let amp = 0.5;
  let f = 1;
  for (let k = 0; k < octaves; k++) {
    const n = noise(x * f, 0.5, seed + k);
    const r = 1 - Math.abs(2 * n - 1);
    sum += amp * r * r;
    f *= 2;
    amp *= 0.5;
  }
  return sum;
}

function mix(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

function clamp(c: number): number {
  return c < 0 ? 0 : c > 1 ? 1 : c;
}

function byte(c: number): number {
  return Math.floor(clamp(c) * 255 + 0.5);
}

/** The colour of a point, as three components, written into `out`. */
function point(x: number, y: number, octaves: number, levels: number, out: Float64Array): void {
  // The sky: zenith to horizon, and clouds where the noise rises.
  const t = clamp(y / 0.7);
  let r = mix(0.2, 0.98, t);
  let g = mix(0.36, 0.8, t);
  let b = mix(0.7, 0.62, t);
  const cloud = clamp((fbm(x * 4, y * 10, 100, octaves) - 0.56) * 3) * (1 - t);
  r = mix(r, 0.97, cloud * 0.85);
  g = mix(g, 0.97, cloud * 0.85);
  b = mix(b, 1, cloud * 0.85);
  // The ranges, far to near: paler with distance, as the air makes them.
  for (let k = 0; k < 3; k++) {
    const top = 0.55 + 0.07 * k - (0.25 - 0.05 * k) * ridge(x * (3 + 3 * k), 200 + k, octaves);
    if (y >= top) {
      const shade = 0.85 + 0.3 * fbm(x * 40, y * 40, 210 + k, octaves);
      const fade = 0.5 - 0.21 * k;
      r = mix((0.55 - 0.175 * k) * shade, 0.98, fade);
      g = mix((0.52 - 0.11 * k) * shade, 0.8, fade);
      b = mix((0.72 - 0.21 * k) * shade, 0.62, fade);
    }
  }
  // The meadow, and its flowers, smaller at each level.
  if (y >= 0.74 + 0.02 * noise(x * 8, 0.5, 350)) {
    const shade = 0.75 + 0.5 * fbm(x * 60, y * 60, 300, octaves);
    r = 0.25 * shade;
    g = 0.45 * shade;
    b = 0.18 * shade;
    let size = 0.012;
    for (let level = 0; level < levels; level++) {
      const fx = Math.floor(x / size);
      const fy = Math.floor(y / size);
      for (let j = -1; j <= 1; j++) {
        for (let i = -1; i <= 1; i++) {
          const cx = (fx | 0) + i;
          const cy = (fy | 0) + j;
          const h = hash(cx, cy, 400 + level);
          if (unit(h) >= 0.55) continue;
          const px = (cx + unit(hash(cx, cy, 500 + level))) * size;
          const py = (cy + unit(hash(cx, cy, 600 + level))) * size;
          const radius = size * (0.18 + 0.12 * unit(hash(cx, cy, 700 + level)));
          const dx = x - px;
          const dy = y - py;
          const heart = radius * 0.3;
          if (dx * dx + dy * dy < heart * heart) {
            r = 0.98;
            g = 0.82;
            b = 0.2;
            continue;
          }
          // Six petals around the heart.
          const petal = radius * 0.45;
          const reach = radius * 0.55;
          const hue = hash(cx, cy, 800 + level) & 3;
          for (let p = 0; p < 6; p++) {
            const ox = p === 0 ? 1 : p === 3 ? -1 : p === 1 || p === 5 ? 0.5 : -0.5;
            const oy = p === 0 || p === 3 ? 0 : p < 3 ? 0.8660254037844386 : -0.8660254037844386;
            const qx = dx - ox * reach;
            const qy = dy - oy * reach;
            if (qx * qx + qy * qy < petal * petal) {
              r = hue === 0 ? 0.95 : hue === 1 ? 0.9 : hue === 2 ? 0.98 : 0.62;
              g = hue === 0 ? 0.35 : hue === 1 ? 0.9 : hue === 2 ? 0.6 : 0.45;
              b = hue === 0 ? 0.45 : hue === 1 ? 0.95 : hue === 2 ? 0.2 : 0.9;
            }
          }
        }
      }
      size *= 0.5;
    }
  }
  out[0] = r;
  out[1] = g;
  out[2] = b;
}

/**
 * Draws the window of `width` × `height` pixels zoomed `zoom` times around
 * (cx, cy); gives the number of pixels drawn. Each doubling of the zoom adds
 * an octave and a level of flowers.
 */
export function render(
  pixels: Uint32Array,
  width: number,
  height: number,
  zoom: number,
  cx: number,
  cy: number,
): number {
  let level = 0;
  let z = zoom;
  while (z >= 2) {
    z /= 2;
    level += 1;
  }
  const octaves = 6 + level;
  const levels = 1 + level;
  const out = new Float64Array(3);
  for (let py = 0; py < height; py++) {
    const y = cy + ((py + 0.5) / height - 0.5) / zoom;
    for (let px = 0; px < width; px++) {
      const x = cx + ((px + 0.5) / width - 0.5) / zoom;
      point(x, y, octaves, levels, out);
      pixels[py * width + px] =
        (byte(out[0] ?? 0) | (byte(out[1] ?? 0) << 8) | (byte(out[2] ?? 0) << 16) | 0xff000000) >>>
        0;
    }
  }
  return width * height;
}
