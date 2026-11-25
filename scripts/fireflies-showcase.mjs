// The fireflies of decision 0043, as an animation for the README: the pond of
// W2 (bench/arena/frame.peras), ten thousand fireflies moving as the workload
// moves them, frame after frame, each as bright as the neighbours it gathers
// that frame; one firefly at a time draws threads of light to its neighbours,
// then lets them go: the short lists the workload makes and forgets, which an
// arena is meant to release at once. Beneath, the time and the memory of each
// option measured, read from bench/arena/RESULTS.md.
//
// The positions and the neighbours are computed here, by the workload's own
// rules; before drawing, they are checked against the module: the same rules,
// run in JavaScript, give the total frame.peras gives, bit for bit. Only the
// twinkle of each firefly is an ornament; it changes no figure.
//
// Usage: node scripts/fireflies-showcase.mjs
// Writes docs/assets/fireflies.webp, and .fr.webp with French labels,
// animated.
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  openSync,
  closeSync,
  readFileSync,
  rmSync,
  writeFileSync,
  writeSync,
} from "node:fs";
import { Buffer } from "node:buffer";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const root = join(import.meta.dirname, "..");
const work = join(root, "target", "fireflies-showcase");
const perasc = join(root, "target", "release", "perasc");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/** @param {string} cmd @param {string[]} args */
function sh(cmd, args) {
  const r = spawnSync(cmd, args, { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`${cmd}: ${r.stderr.slice(-800)}`);
  return r;
}

const SIDE = 64;

/** The fireflies of W2 at its first frame, as frame.peras places them. @param {number} n */
function place(n) {
  const xs = new Float64Array(n);
  const ys = new Float64Array(n);
  let seed = 12345;
  for (let i = 0; i < n; i++) {
    seed = (Math.imul(seed, 1103515245) + 12345) | 0;
    xs[i] = ((seed >> 8) & 65535) / 65536;
    seed = (Math.imul(seed, 1103515245) + 12345) | 0;
    ys[i] = ((seed >> 8) & 65535) / 65536;
  }
  return { xs, ys };
}

/** @param {number} v */
const cell = (v) => Math.min(Math.floor(v * SIDE), SIDE - 1);

/**
 * One frame of W2, as frame.peras computes it, in its order: each firefly's
 * neighbours, the frame's share of the total, and the list one firefly makes.
 * @param {Float64Array} xs @param {Float64Array} ys @param {number} keep
 */
function frame(xs, ys, keep = -1) {
  const n = xs.length;
  /** @type {number[][]} */
  const cells = Array.from({ length: SIDE * SIDE }, () => []);
  for (let i = 0; i < n; i++) cells[cell(ys[i] ?? 0) * SIDE + cell(xs[i] ?? 0)]?.push(i);
  const near = new Int32Array(n);
  /** @type {number[]} */
  let kept = [];
  let total = 0;
  for (let i = 0; i < n; i++) {
    const cx = cell(xs[i] ?? 0);
    const cy = cell(ys[i] ?? 0);
    /** @type {number[]} */
    const list = [];
    for (let dy = -1; dy <= 1; dy++)
      for (let dx = -1; dx <= 1; dx++) {
        const x = cx + dx;
        const y = cy + dy;
        if (x >= 0 && x < SIDE && y >= 0 && y < SIDE)
          for (const j of cells[y * SIDE + x] ?? []) if (j !== i) list.push(j);
      }
    let px = 0;
    let py = 0;
    for (const j of list) {
      px += (xs[j] ?? 0) - (xs[i] ?? 0);
      py += (ys[j] ?? 0) - (ys[i] ?? 0);
    }
    total += px + py + list.length;
    near[i] = list.length;
    if (i === keep) kept = list;
  }
  return { near, total, kept };
}

/** The move of every firefly between two frames, as frame.peras makes it. @param {Float64Array} xs @param {Float64Array} ys */
function move(xs, ys) {
  for (let i = 0; i < xs.length; i++) {
    xs[i] = ((xs[i] ?? 0) + 0.0037) % 1;
    ys[i] = ((ys[i] ?? 0) + 0.0021) % 1;
  }
}

/** W2's result, computed here. @param {number} n @param {number} frames */
function run(n, frames) {
  const { xs, ys } = place(n);
  let total = 0;
  for (let f = 0; f < frames; f++) {
    total += frame(xs, ys).total;
    move(xs, ys);
  }
  return total;
}

// 1. The rules here are the workload's: the same total as the module's.
sh(perasc, [
  "build",
  join(root, "bench", "arena", "frame.peras"),
  "--out",
  join(work, "frame"),
  "--allocator",
  "tlsf",
  "--color",
  "never",
]);
/** @type {unknown} */
const loaded = await import(pathToFileURL(join(work, "frame", "frame.js")).href);
const module = /** @type {{ run(n: number, frames: number): number }} */ (loaded);
for (const [n, frames] of [
  [2000, 3],
  [10000, 2],
]) {
  const want = module.run(n ?? 0, frames ?? 0);
  const got = run(n ?? 0, frames ?? 0);
  if (want !== got)
    throw new Error(`n ${String(n)}: the module gives ${String(want)}, here ${String(got)}`);
}

// 2. The figures, read from the results.
const results = readFileSync(join(root, "bench", "arena", "RESULTS.md"), "utf8");
const w2 = results.split("### W2")[1]?.split("###")[0] ?? "";
/** @type {{ option: string, ratio: number, memory: number }[]} */
const rows = [];
for (const line of w2.split("\n")) {
  const m = /^\| (a\d\w*),[^|]*\|[^|]*\| ([\d.]+) \([^|]*\|[^|]*\|\s*(\d+) KiB/.exec(line);
  if (m) rows.push({ option: m[1] ?? "", ratio: Number(m[2]), memory: Number(m[3]) });
}
const a0 = rows.find((r) => r.option === "a0");
if (rows.length !== 5 || a0 === undefined) throw new Error("W2's table not read");

// 3. The scene: a title band, the pond, the figures.
const W = 960;
const TOP = 106;
const H = 540;
const P = 230;
const FULL = TOP + H + P;
const N = 10000;
const FRAMES = 36;
const CYCLE = 12;
const pond = { x: 16, y: TOP, w: W - 32, h: H };

// The still parts, drawn once: night water, its ripples, the grid built again
// each frame, faintly; the panel of figures.
const still = new Float32Array(W * FULL * 3);
for (let y = 0; y < FULL; y++)
  for (let x = 0; x < W; x++) {
    const o = (y * W + x) * 3;
    let r = 6;
    let g = 9;
    let b = 18;
    if (y >= pond.y && y < pond.y + pond.h && x >= pond.x && x < pond.x + pond.w) {
      const u = (x - pond.x) / pond.w - 0.5;
      const v = (y - pond.y) / pond.h - 0.5;
      const vignette = 1 - Math.min(1, (u * u + v * v) * 1.6);
      const ripple = Math.sin(x * 0.012 + Math.sin(y * 0.009) * 2.0);
      r = 3 + 5 * vignette + ripple;
      g = 9 + 14 * vignette + ripple * 1.5;
      b = 18 + 22 * vignette + ripple * 2;
      const gx = ((x - pond.x) / pond.w) * SIDE;
      const gy = ((y - pond.y) / pond.h) * SIDE;
      if (Math.abs(gx - Math.round(gx)) < 0.03 || Math.abs(gy - Math.round(gy)) < 0.03) {
        g += 5;
        b += 8;
      }
    } else if (y >= TOP + H) {
      r = 9;
      g = 11;
      b = 20;
    }
    still[o] = r;
    still[o + 1] = g;
    still[o + 2] = b;
  }

/** @param {number} x0 @param {number} y0 @param {number} w @param {number} h @param {number[]} rgb */
function box(x0, y0, w, h, rgb) {
  for (let y = Math.round(y0); y < y0 + h; y++)
    for (let x = Math.round(x0); x < x0 + w; x++) {
      const o = (y * W + x) * 3;
      still[o] = rgb[0] ?? 0;
      still[o + 1] = rgb[1] ?? 0;
      still[o + 2] = rgb[2] ?? 0;
    }
}
const barsTop = TOP + H + 52;
const rowH = 30;
const span = 300;
const panels = [
  { x: 100, key: "ratio", max: 1.3, limit: 1.0 },
  { x: 580, key: "memory", max: 2.4, limit: 1.1 },
];
for (const p of panels) {
  rows.forEach((row, k) => {
    const v = p.key === "ratio" ? row.ratio : row.memory / a0.memory;
    const over = p.key === "ratio" ? v > 1 : v > 1.1;
    const color = row.option === "a0" ? [140, 165, 205] : over ? [226, 118, 92] : [244, 214, 112];
    box(p.x, barsTop + k * rowH, (v / p.max) * span, rowH - 10, color);
  });
  box(p.x + (p.limit / p.max) * span, barsTop - 8, 2, rows.length * rowH + 4, [255, 255, 255]);
}

// 4. The frames: the fireflies where the workload puts them, frame after
// frame; one of them gathering its neighbours, reading them, forgetting them,
// seen again through a lens four times larger.
const raw = join(work, "fireflies.rgb");
const fd = openSync(raw, "w");
const { xs, ys } = place(N);
const light = new Float32Array(W * FULL * 3);
const LENS = { x: W - 150, y: TOP + H - 150, r: 120, zoom: 4 };
/**
 * Light added around a point, inside a clip.
 * @param {number} x @param {number} y @param {number} a @param {number} radius
 * @param {number[]} rgb @param {(px: number, py: number) => boolean} inside
 */
function glow(x, y, a, radius, rgb, inside) {
  const reach = Math.ceil(radius * 2.4);
  for (let py = Math.floor(y) - reach; py <= y + reach; py++)
    for (let px = Math.floor(x) - reach; px <= x + reach; px++) {
      if (!inside(px, py)) continue;
      const d2 = ((px - x) ** 2 + (py - y) ** 2) / (radius * radius);
      const s = a * Math.exp(-d2);
      const o = (py * W + px) * 3;
      light[o] = (light[o] ?? 0) + s * (rgb[0] ?? 0);
      light[o + 1] = (light[o + 1] ?? 0) + s * (rgb[1] ?? 0);
      light[o + 2] = (light[o + 2] ?? 0) + s * (rgb[2] ?? 0);
    }
}
const inLens = (/** @type {number} */ px, /** @type {number} */ py) =>
  (px - LENS.x) ** 2 + (py - LENS.y) ** 2 < (LENS.r - 2) ** 2;
const inPond = (/** @type {number} */ px, /** @type {number} */ py) =>
  px >= pond.x && px < pond.x + pond.w && py >= pond.y && py < pond.y + pond.h && !inLens(px, py);
const at = (/** @type {number} */ i) => ({
  x: pond.x + (xs[i] ?? 0) * pond.w,
  y: pond.y + (ys[i] ?? 0) * pond.h,
});
const WARM = [1.0, 0.86, 0.42];
const HALO = [0.75, 0.95, 0.35];
const THREAD = [0.55, 0.9, 1.0];
let low = Infinity;
let high = 0;
for (let f = 0; f < FRAMES; f++) {
  // The firefly that gathers, a new one each cycle, among the crowded, away
  // from the lens and the edges.
  const c = Math.floor(f / CYCLE);
  const { near } = frame(xs, ys);
  let focus = (Math.imul(c + 7, 2654435761) >>> 0) % N;
  for (;;) {
    const p = at(focus);
    const clear =
      (near[focus] ?? 0) >= 26 &&
      p.x > pond.x + 60 &&
      p.y > pond.y + 60 &&
      p.x < LENS.x - LENS.r - 40 &&
      p.y < pond.y + pond.h - 60;
    if (clear) break;
    focus = (focus + 1) % N;
  }
  const { kept } = frame(xs, ys, focus);
  light.fill(0);
  for (let i = 0; i < N; i++) {
    const k = near[i] ?? 0;
    low = Math.min(low, k);
    high = Math.max(high, k);
    const bright = 0.14 + 0.86 * Math.min(k / 40, 1) ** 1.6;
    const twinkle = 0.75 + 0.25 * Math.sin(i * 2.399 + f * 0.55);
    const { x, y } = at(i);
    glow(x, y, 0.05 * bright * twinkle, 3.2, HALO, inPond);
    glow(x, y, 1.2 * bright * twinkle, 0.9, WARM, inPond);
  }
  // Threads to the neighbours: drawn out, held, let go.
  const step = f % CYCLE;
  const reach = Math.min(1, (step + 1) / 4);
  const fade = step < 8 ? 1 : (CYCLE - step) / 4;
  const from = at(focus);
  /** @param {{x: number, y: number}} p */
  const lens = (p) => ({
    x: LENS.x + (p.x - from.x) * LENS.zoom,
    y: LENS.y + (p.y - from.y) * LENS.zoom,
  });
  // In the lens: the fireflies near the one that gathers, four times larger,
  // the 3 by 3 cells it searches, and its threads.
  for (let i = 0; i < N; i++) {
    const p = at(i);
    if (
      Math.abs(p.x - from.x) > LENS.r / LENS.zoom + 4 ||
      Math.abs(p.y - from.y) > LENS.r / LENS.zoom + 4
    )
      continue;
    const k = near[i] ?? 0;
    const bright = 0.06 + 0.94 * Math.min(k / 40, 1) ** 1.6;
    const q = lens(p);
    glow(q.x, q.y, 0.12 * bright, 9, HALO, inLens);
    glow(q.x, q.y, 1.0 * bright, 2.4, WARM, inLens);
  }
  const cw = (pond.w / SIDE) * LENS.zoom;
  const ch = (pond.h / SIDE) * LENS.zoom;
  const fx = Math.min(Math.floor((xs[focus] ?? 0) * SIDE), SIDE - 1);
  const fy = Math.min(Math.floor((ys[focus] ?? 0) * SIDE), SIDE - 1);
  const left = lens({ x: pond.x + ((fx - 1) * pond.w) / SIDE, y: 0 }).x;
  const top = lens({ x: 0, y: pond.y + ((fy - 1) * pond.h) / SIDE }).y;
  for (let s = 0; s <= 3; s++) {
    for (let t = 0; t <= 3 * cw; t += 1)
      glow(left + t, top + s * ch, 0.05, 0.6, [0.6, 0.8, 1], inLens);
    for (let t = 0; t <= 3 * ch; t += 1)
      glow(left + s * cw, top + t, 0.05, 0.6, [0.6, 0.8, 1], inLens);
  }
  for (const j of kept) {
    const to = at(j);
    const a = lens(from);
    const b = lens(to);
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    for (let s = 0; s <= len * reach; s += 0.8) {
      const t = s / Math.max(len, 1);
      glow(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, 0.12 * fade, 0.9, THREAD, inLens);
    }
    glow(b.x, b.y, 0.8 * fade * reach, 4, THREAD, inLens);
    // and on the pond, small.
    for (let s = 0; s <= Math.hypot(to.x - from.x, to.y - from.y) * reach; s += 0.6) {
      const t = s / Math.max(Math.hypot(to.x - from.x, to.y - from.y), 1);
      glow(
        from.x + (to.x - from.x) * t,
        from.y + (to.y - from.y) * t,
        0.1 * fade,
        0.6,
        THREAD,
        inPond,
      );
    }
  }
  glow(lens(from).x, lens(from).y, 1.6 * fade + 0.4, 6, [1, 1, 0.8], inLens);
  glow(from.x, from.y, 1.0 * fade + 0.3, 2.4, [1, 1, 0.8], inPond);
  // The lens's rim, and a thread from it to the firefly it follows.
  for (let a = 0; a < Math.PI * 2; a += 0.004) {
    const x = LENS.x + Math.cos(a) * LENS.r;
    const y = LENS.y + Math.sin(a) * LENS.r;
    glow(x, y, 0.35, 0.9, [0.8, 0.9, 1], () => true);
  }
  const dx = from.x - LENS.x;
  const dy = from.y - LENS.y;
  const d = Math.hypot(dx, dy);
  for (let s = LENS.r; s < d - 8; s += 1.5)
    glow(LENS.x + (dx / d) * s, LENS.y + (dy / d) * s, 0.14, 0.7, [0.8, 0.9, 1], inPond);

  const pixels = Buffer.alloc(W * FULL * 3);
  const tone = (/** @type {number} */ v) => 1 - Math.exp(-v * 2.4);
  for (let o = 0; o < pixels.length; o++) {
    const p = Math.floor(o / 3);
    const dim = inLens(p % W, Math.floor(p / W)) ? 0.45 : 1;
    pixels[o] = Math.min(255, (still[o] ?? 0) * dim + 255 * tone(light[o] ?? 0));
  }
  writeSync(fd, pixels);
  move(xs, ys);
}
closeSync(fd);

/** @param {string} lang */
function draw(lang) {
  const fr = lang === "fr";
  const font = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
  const bold = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
  const number = (/** @type {number} */ v) => (fr ? v.toFixed(3).replace(".", ",") : v.toFixed(3));
  /** @type {[string, string, number, number, number, string][]} */
  const texts = [
    [fr ? "Les lucioles" : "The fireflies", bold, 20, 18, 24, "white"],
    [
      fr
        ? "À chaque image, chacune rassemble ses voisines, les lit, puis les oublie."
        : "Each frame, each one gathers its neighbours, reads them, then forgets them.",
      font,
      20,
      52,
      15,
      "white@0.9",
    ],
    [
      fr
        ? `10 000 lucioles · éclat : les voisines rassemblées, de ${String(low)} à ${String(high)} · loupe ×4 : une liste faite, lue, rendue`
        : `10,000 fireflies · brightness: the neighbours gathered, ${String(low)} to ${String(high)} · lens ×4: one list made, read, released`,
      font,
      20,
      78,
      13,
      "white@0.7",
    ],
    [
      fr ? "Temps d’une image, face à a0" : "Time of a frame, against a0",
      bold,
      100,
      TOP + H + 18,
      15,
      "white",
    ],
    [
      fr
        ? "Mémoire à la fin, face à a0 (plafond 1,10)"
        : "Memory at the end, against a0 (limit 1.10)",
      bold,
      580,
      TOP + H + 18,
      15,
      "white",
    ],
    [
      fr
        ? "a0 gardé : aucune option ne tient les critères enregistrés avant la mesure (décision 0043)"
        : "a0 kept: no option meets the criteria registered before the measure (decision 0043)",
      font,
      100,
      FULL - 26,
      13,
      "white@0.8",
    ],
  ];
  rows.forEach((row, k) => {
    const y = barsTop + k * rowH + 2;
    const mem = row.memory / a0.memory;
    for (const p of panels) {
      const v = p.key === "ratio" ? row.ratio : mem;
      const end = Math.max(v / p.max, p.limit / p.max) * span;
      texts.push([row.option, bold, p.x - 52, y, 14, "white"]);
      texts.push([number(v), font, p.x + end + 10, y, 14, "white@0.9"]);
    }
  });
  const filters = texts.map(([text, f, x, y, size, color], i) => {
    const file = join(work, `${lang}-${String(i)}.txt`);
    writeFileSync(file, text);
    return `drawtext=fontfile=${f}:textfile=${file}:x=${String(x)}:y=${String(y)}:fontsize=${String(size)}:fontcolor=${color}`;
  });
  const out = join(root, "docs", "assets", fr ? "fireflies.fr.webp" : "fireflies.webp");
  sh("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    "-f",
    "rawvideo",
    "-pixel_format",
    "rgb24",
    "-video_size",
    `${String(W)}x${String(FULL)}`,
    "-framerate",
    "12",
    "-i",
    raw,
    "-vf",
    filters.join(","),
    "-c:v",
    "libwebp_anim",
    "-loop",
    "0",
    "-quality",
    "50",
    out,
  ]);
  return out;
}

for (const lang of ["en", "fr"]) console.log(draw(lang));
console.log(
  `W2 checked against frame.peras; ${String(N)} fireflies over ${String(FRAMES)} frames, ${String(low)} to ${String(high)} neighbours each; ${rows.map((r) => `${r.option} ${String(r.ratio)}`).join(", ")}.`,
);
