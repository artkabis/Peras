// The garden of Apeiron (decision 0013), written as TypeScript is often
// written: a new object for each colour, each neighbouring cell and each
// petal, octaves gathered with Array.from and reduce. The same arithmetic as
// bench/garden/garden.ts, in the same order, so the same pixels, bit for bit:
// scripts/candle.mjs checks it before it measures anything.

interface Color {
  r: number;
  g: number;
  b: number;
}

interface Cell {
  cx: number;
  cy: number;
}

const hash = (x: number, y: number, seed: number): number => {
  let h = (Math.imul(x, 374761393) + Math.imul(y, 668265263) + Math.imul(seed, 1274126177)) | 0;
  h ^= h >>> 16;
  h = Math.imul(h, 2146121005);
  h ^= h >>> 15;
  h = Math.imul(h, -2073254261);
  h ^= h >>> 16;
  return h;
};

const unit = (h: number): number => (h & 16777215) / 16777216;

const noise = (x: number, y: number, seed: number): number => {
  const fx = Math.floor(x);
  const fy = Math.floor(y);
  const ix = fx | 0;
  const iy = fy | 0;
  const tx = x - fx;
  const ty = y - fy;
  const ux = tx * tx * (3 - 2 * tx);
  const uy = ty * ty * (3 - 2 * ty);
  const [a, b, c, d] = [
    { i: ix, j: iy },
    { i: ix + 1, j: iy },
    { i: ix, j: iy + 1 },
    { i: ix + 1, j: iy + 1 },
  ].map((corner) => unit(hash(corner.i, corner.j, seed))) as [number, number, number, number];
  return a + (b - a) * ux + (c - a) * uy + (a - b - c + d) * ux * uy;
};

const octavesOf = (octaves: number): { f: number; amp: number; k: number }[] =>
  Array.from({ length: octaves }, (_, k) => ({ f: 2 ** k, amp: 0.5 ** (k + 1), k }));

const fbm = (x: number, y: number, seed: number, octaves: number): number =>
  octavesOf(octaves)
    .map((o) => o.amp * noise(x * o.f, y * o.f, seed + o.k))
    .reduce((sum, v) => sum + v, 0);

const ridge = (x: number, seed: number, octaves: number): number =>
  octavesOf(octaves)
    .map((o) => {
      const n = noise(x * o.f, 0.5, seed + o.k);
      const r = 1 - Math.abs(2 * n - 1);
      return o.amp * r * r;
    })
    .reduce((sum, v) => sum + v, 0);

const mix = (a: number, b: number, t: number): number => a + (b - a) * t;
const clamp = (c: number): number => (c < 0 ? 0 : c > 1 ? 1 : c);
const byte = (c: number): number => Math.floor(clamp(c) * 255 + 0.5);
const mixColor = (a: Color, b: Color, t: number): Color => ({
  r: mix(a.r, b.r, t),
  g: mix(a.g, b.g, t),
  b: mix(a.b, b.b, t),
});

const HEART: Color = { r: 0.98, g: 0.82, b: 0.2 };
const HUES: Color[] = [
  { r: 0.95, g: 0.35, b: 0.45 },
  { r: 0.9, g: 0.9, b: 0.95 },
  { r: 0.98, g: 0.6, b: 0.2 },
  { r: 0.62, g: 0.45, b: 0.9 },
];

const neighbours = (fx: number, fy: number): Cell[] =>
  [-1, 0, 1].flatMap((j) => [-1, 0, 1].map((i) => ({ cx: (fx | 0) + i, cy: (fy | 0) + j })));

const petals = (): { ox: number; oy: number }[] =>
  [0, 1, 2, 3, 4, 5].map((p) => ({
    ox: p === 0 ? 1 : p === 3 ? -1 : p === 1 || p === 5 ? 0.5 : -0.5,
    oy: p === 0 || p === 3 ? 0 : p < 3 ? 0.8660254037844386 : -0.8660254037844386,
  }));

const point = (x: number, y: number, octaves: number, levels: number): Color => {
  const t = clamp(y / 0.7);
  let color: Color = { r: mix(0.2, 0.98, t), g: mix(0.36, 0.8, t), b: mix(0.7, 0.62, t) };
  const cloud = clamp((fbm(x * 4, y * 10, 100, octaves) - 0.56) * 3) * (1 - t);
  color = mixColor(color, { r: 0.97, g: 0.97, b: 1 }, cloud * 0.85);
  [0, 1, 2].forEach((k) => {
    const top = 0.55 + 0.07 * k - (0.25 - 0.05 * k) * ridge(x * (3 + 3 * k), 200 + k, octaves);
    if (y >= top) {
      const shade = 0.85 + 0.3 * fbm(x * 40, y * 40, 210 + k, octaves);
      const fade = 0.5 - 0.21 * k;
      color = mixColor(
        {
          r: (0.55 - 0.175 * k) * shade,
          g: (0.52 - 0.11 * k) * shade,
          b: (0.72 - 0.21 * k) * shade,
        },
        { r: 0.98, g: 0.8, b: 0.62 },
        fade,
      );
    }
  });
  if (y >= 0.74 + 0.02 * noise(x * 8, 0.5, 350)) {
    const shade = 0.75 + 0.5 * fbm(x * 60, y * 60, 300, octaves);
    color = { r: 0.25 * shade, g: 0.45 * shade, b: 0.18 * shade };
    Array.from({ length: levels }, (_, level) => ({ level, size: 0.012 * 0.5 ** level })).forEach(
      ({ level, size }) => {
        neighbours(Math.floor(x / size), Math.floor(y / size)).forEach(({ cx, cy }) => {
          if (unit(hash(cx, cy, 400 + level)) >= 0.55) return;
          const px = (cx + unit(hash(cx, cy, 500 + level))) * size;
          const py = (cy + unit(hash(cx, cy, 600 + level))) * size;
          const radius = size * (0.18 + 0.12 * unit(hash(cx, cy, 700 + level)));
          const dx = x - px;
          const dy = y - py;
          const heart = radius * 0.3;
          if (dx * dx + dy * dy < heart * heart) {
            color = { ...HEART };
            return;
          }
          const petal = radius * 0.45;
          const reach = radius * 0.55;
          const hue = HUES[hash(cx, cy, 800 + level) & 3] ?? HEART;
          petals().forEach(({ ox, oy }) => {
            const qx = dx - ox * reach;
            const qy = dy - oy * reach;
            if (qx * qx + qy * qy < petal * petal) color = { ...hue };
          });
        });
      },
    );
  }
  return color;
};

/** Draws the window, as garden.ts does; gives the number of pixels drawn. */
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
  const colors: Color[] = [];
  for (let py = 0; py < height; py++) {
    const y = cy + ((py + 0.5) / height - 0.5) / zoom;
    for (let px = 0; px < width; px++) {
      const x = cx + ((px + 0.5) / width - 0.5) / zoom;
      colors.push(point(x, y, octaves, levels));
    }
  }
  colors.forEach((c, i) => {
    pixels[i] = (byte(c.r) | (byte(c.g) << 8) | (byte(c.b) << 16) | 0xff000000) >>> 0;
  });
  return width * height;
}
