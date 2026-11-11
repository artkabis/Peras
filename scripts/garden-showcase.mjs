// The before and after of the garden of Apeiron (decision 0013), as an image
// for the README: zoomed 8 and 64 times, what the stored image gives, the
// smallest AVIF of the whole scene at SSIM 0.95 that bench-garden measured,
// enlarged; and what the program gives, the scene drawn at that zoom. The
// bytes and SSIM written on the image are computed here, by FFmpeg, from the
// files themselves; the AVIF's setting is read from bench/garden/results.json.
// Usage: node scripts/garden-showcase.mjs
// Writes docs/assets/garden-before-after.webp, and .fr.webp with French labels.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";
import { minify } from "vite";

const root = join(import.meta.dirname, "..");
const work = join(root, "target", "garden-showcase");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
const perasc = join(root, "target", "release", "perasc");
/** @param {string} command @param {string[]} args */
function sh(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 28 });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")}\n${result.stderr}`);
  return result;
}
sh(perasc, [
  "build",
  join(root, "bench", "garden", "garden.peras"),
  "--allocator",
  "tlsf",
  "--out",
  work,
]);
/** @type {unknown} */
const loaded = await import(pathToFileURL(join(work, "garden.js")).href);
const peras =
  /** @type {{ render: (p: unknown, w: number, h: number, z: number, x: number, y: number) => number, __perasHeap: { array: (k: string, n: number) => { view: () => Int32Array, release: () => void } } }} */ (
    loaded
  );
/** The garden drawn by Peras, as a PNG; gives its path. */
function draw(
  /** @type {string} */ name,
  /** @type {number} */ side,
  /** @type {number} */ zoom,
  /** @type {number} */ x,
  /** @type {number} */ y,
) {
  const array = peras.__perasHeap.array("i32", side * side);
  peras.render(array, side, side, zoom, x, y);
  const raw = join(work, `${name}.rgba`);
  writeFileSync(raw, new Uint8Array(array.view().slice().buffer));
  array.release();
  const out = join(work, `${name}.png`);
  sh("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    "-f",
    "rawvideo",
    "-pix_fmt",
    "rgba",
    "-s",
    `${String(side)}x${String(side)}`,
    "-i",
    raw,
    "-frames:v",
    "1",
    out,
  ]);
  return out;
}

// The stored image: the whole scene, 1024 × 1024, as the smallest AVIF at
// SSIM 0.95 the benchmark found.
/** @typedef {{ h1Avif: { chroma: string, bytes: number, crf: number }[], shipped: Record<string, number> }} Results */
const resultsJson = /** @type {unknown} */ (
  JSON.parse(readFileSync(join(root, "bench", "garden", "results.json"), "utf8"))
);
const results = /** @type {Results} */ (resultsJson);
const best = results.h1Avif.reduce((a, b) => (b.bytes < a.bytes ? b : a));
const whole = draw("whole", 1024, 1, 0.5, 0.5);
const avif = join(work, "whole.avif");
sh("ffmpeg", [
  "-y",
  "-loglevel",
  "error",
  "-i",
  whole,
  "-c:v",
  "libaom-av1",
  "-still-picture",
  "1",
  "-crf",
  String(best.crf),
  "-cpu-used",
  "6",
  "-pix_fmt",
  best.chroma,
  avif,
]);
const avifBytes = statSync(avif).size;
// The program as shipped: its loader minified, and its module, gzip.
const programBytes =
  gzipSync((await minify("garden.js", readFileSync(join(work, "garden.js"), "utf8"))).code, {
    level: 9,
  }).length + gzipSync(readFileSync(join(work, "garden.wasm")), { level: 9 }).length;

const panel = 480;
const focus = { x: 0.37, y: 0.83 };
/** @type {{ zoom: number, stored: string, drawn: string, ssim: number }[]} */
const rows = [];
for (const zoom of [8, 64]) {
  // The window of the stored image, enlarged by Lanczos as a viewer would.
  const size = 1024 / zoom;
  const x0 = Math.round((focus.x - 0.5 / zoom) * 1024);
  const y0 = Math.round((focus.y - 0.5 / zoom) * 1024);
  const stored = join(work, `stored-${String(zoom)}.png`);
  sh("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    "-i",
    avif,
    "-vf",
    `crop=${String(size)}:${String(size)}:${String(x0)}:${String(y0)},scale=${String(panel)}:${String(panel)}:flags=lanczos,format=rgb24`,
    stored,
  ]);
  // The program, drawing the same window at this zoom.
  const drawn = draw(
    `drawn-${String(zoom)}`,
    panel,
    zoom,
    (x0 + size / 2) / 1024,
    (y0 + size / 2) / 1024,
  );
  const r = sh("ffmpeg", [
    "-hide_banner",
    "-i",
    stored,
    "-i",
    drawn,
    "-lavfi",
    "[0:v]format=yuv444p[a];[1:v]format=yuv444p[b];[a][b]ssim",
    "-f",
    "null",
    "-",
  ]);
  rows.push({ zoom, stored, drawn, ssim: Number(/All:([0-9.]+)/.exec(r.stderr)?.[1] ?? "0") });
}

const font = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
const bold = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
const kb = (/** @type {number} */ n, /** @type {string} */ point) =>
  `${(n / 1024).toFixed(1).replace(".", point)} KB`;
const languages = /** @type {const} */ ([
  {
    file: "garden-before-after.webp",
    point: ".",
    stored: (/** @type {number} */ z, /** @type {number} */ s) => [
      `Before: the stored image, zoomed ×${String(z)}`,
      `AVIF of the whole scene, ${kb(avifBytes, ".")}  ·  SSIM ${s.toFixed(2)} against the drawing`,
    ],
    drawn: (/** @type {number} */ z) => [
      `After: the program, zoomed ×${String(z)}`,
      `Peras, ${kb(programBytes, ".")}  ·  drawn at this zoom, new detail`,
    ],
  },
  {
    file: "garden-before-after.fr.webp",
    point: ",",
    stored: (/** @type {number} */ z, /** @type {number} */ s) => [
      `Avant : l’image stockée, zoomée ×${String(z)}`,
      `AVIF de la scène entière, ${kb(avifBytes, ",")}  ·  SSIM ${s.toFixed(2).replace(".", ",")} contre le dessin`,
    ],
    drawn: (/** @type {number} */ z) => [
      `Après : le programme, zoomé ×${String(z)}`,
      `Peras, ${kb(programBytes, ",")}  ·  dessiné à ce zoom, des détails nouveaux`,
    ],
  },
]);
/** Text for drawtext, its special characters escaped. */
const text = (/** @type {string} */ s) =>
  s.replace(/\\/g, "\\\\").replace(/:/g, "\\:").replace(/'/g, "\u2019").replace(/%/g, "\\%");
for (const lang of languages) {
  const inputs = rows.flatMap((r) => ["-i", r.stored, "-i", r.drawn]);
  const label = (/** @type {number} */ i, /** @type {string[]} */ lines) =>
    `[${String(i)}:v]drawbox=x=0:y=0:w=iw:h=58:color=black@0.62:t=fill,` +
    `drawtext=fontfile=${bold}:text='${text(lines[0] ?? "")}':x=12:y=10:fontsize=19:fontcolor=white,` +
    `drawtext=fontfile=${font}:text='${text(lines[1] ?? "")}':x=12:y=35:fontsize=14:fontcolor=white@0.9[p${String(i)}]`;
  const filters = rows.flatMap((r, k) => [
    label(2 * k, lang.stored(r.zoom, r.ssim)),
    label(2 * k + 1, lang.drawn(r.zoom)),
  ]);
  const grid = `${filters.join(";")};[p0][p1]hstack=inputs=2[top];[p2][p3]hstack=inputs=2[bottom];[top][bottom]vstack=inputs=2`;
  sh("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    ...inputs,
    "-filter_complex",
    grid,
    "-c:v",
    "libwebp",
    "-quality",
    "85",
    "-compression_level",
    "6",
    join(root, "docs", "assets", lang.file),
  ]);
}
console.log(
  `Stored: AVIF ${String(avifBytes)} bytes (${best.chroma}, crf ${String(best.crf)}); program: ${String(programBytes)} bytes. SSIM of the stored image against the drawing: ${rows.map((r) => `×${String(r.zoom)} ${r.ssim.toFixed(3)}`).join(", ")}.`,
);
