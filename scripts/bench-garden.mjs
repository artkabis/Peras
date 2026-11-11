// The garden of Apeiron (decision 0013), measured under its registered
// protocol: the same scene in TypeScript (bench/garden/garden.ts), Peras
// (garden.peras, compiled to WebAssembly) and Rust (rust/, wasm-bindgen).
//   1. Same pixels: SHA-256 of each version's render at zooms 1, 4, 16, 64.
//   2. Bytes shipped by each version, minified by Vite where it applies, gzip.
//   3. H1, H2, H4: PNG, WebP and AVIF of the renders, encoded by FFmpeg; their
//      SSIM and PSNR against the procedural render, computed by FFmpeg.
//   4. H3, H5: milliseconds per 1024 × 1024 frame in Chromium, medians of 15
//      runs after 2 of warm-up, the versions alternating.
// Usage: node scripts/bench-garden.mjs [--check] [--no-rust]
//        --check: the versions give the same pixels, at 256 × 256; nothing
//        else is measured or written. Rust is left out where cargo or
//        wasm-bindgen is missing.
// Writes bench/garden/RESULTS.md and bench/garden/results.json, unless --check.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";
import { chromium } from "playwright";
import ts from "typescript";
import { minify } from "vite";

const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "garden");
const check = process.argv.includes("--check");
const perasc = join(root, "target", "release", "perasc");
const work = join(root, "target", "bench-garden");
rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "ts"), { recursive: true });
mkdirSync(join(work, "img"), { recursive: true });

/** @param {string} command @param {string[]} args @param {object} [options] */
function sh(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 28, ...options });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  return result;
}
/** @param {string} command */
const has = (command) => spawnSync(command, ["--version"], { encoding: "utf8" }).status === 0;

// 1. The versions.
const tsSource = readFileSync(join(dir, "garden.ts"), "utf8");
const tsJs = ts.transpileModule(tsSource, {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
}).outputText;
writeFileSync(join(work, "ts", "garden.js"), tsJs);
sh(perasc, [
  "build",
  join(dir, "garden.peras"),
  "--allocator",
  "tlsf",
  "--out",
  join(work, "peras"),
]);
sh(perasc, ["build", join(dir, "garden.peras"), "--out", join(work, "debug")]);
const withRust = !process.argv.includes("--no-rust") && has("cargo") && has("wasm-bindgen");
if (withRust) {
  const crate = join(dir, "rust");
  sh("cargo", ["build", "--release", "--locked", "--target", "wasm32-unknown-unknown", "-q"], {
    cwd: crate,
    env: { ...process.env, RUSTFLAGS: "-C target-feature=+simd128" },
  });
  sh("wasm-bindgen", [
    join(crate, "target", "wasm32-unknown-unknown", "release", "bench_garden_rust.wasm"),
    ...["--target", "web", "--out-dir", join(work, "rust")],
  ]);
}

/** @typedef {(width: number, height: number, zoom: number, cx: number, cy: number) => Uint32Array} Render */
/** @type {Record<string, Render>} */
const renders = {};
{
  /** @type {unknown} */
  const tsModule = await import(pathToFileURL(join(work, "ts", "garden.js")).href);
  const t =
    /** @type {{ render: (p: Uint32Array, w: number, h: number, z: number, x: number, y: number) => number }} */ (
      tsModule
    );
  renders.typescript = (w, h, z, x, y) => {
    const pixels = new Uint32Array(w * h);
    t.render(pixels, w, h, z, x, y);
    return pixels;
  };
  for (const [name, folder] of [
    ["peras", "peras"],
    ["peras-debug", "debug"],
  ]) {
    /** @type {unknown} */
    const loaded = await import(pathToFileURL(join(work, folder, "garden.js")).href);
    const p =
      /** @type {{ render: (p: unknown, w: number, h: number, z: number, x: number, y: number) => number, __perasHeap: { array: (k: string, n: number) => { view: () => Int32Array, release: () => void } } }} */ (
        loaded
      );
    /** @type {Render} */
    const render = (w, h, z, x, y) => {
      const array = p.__perasHeap.array("i32", w * h);
      p.render(array, w, h, z, x, y);
      const out = new Uint32Array(array.view().slice().buffer);
      array.release();
      return out;
    };
    renders[name] = render;
  }
  if (withRust) {
    /** @type {unknown} */
    const loaded = await import(pathToFileURL(join(work, "rust", "bench_garden_rust.js")).href);
    const r =
      /** @type {{ default: (o: { module_or_path: Buffer }) => Promise<{ memory: { buffer: ArrayBuffer } }>, Pixels: new (n: number) => { ptr: () => number, free: () => void }, render: (p: unknown, w: number, h: number, z: number, x: number, y: number) => number }} */ (
        loaded
      );
    const wasm = await r.default({
      module_or_path: readFileSync(join(work, "rust", "bench_garden_rust_bg.wasm")),
    });
    renders.rust = (w, h, z, x, y) => {
      const pixels = new r.Pixels(w * h);
      r.render(pixels, w, h, z, x, y);
      const out = new Uint32Array(wasm.memory.buffer, pixels.ptr(), w * h).slice();
      pixels.free();
      return out;
    };
  }
}

// 2. The same pixels, at every zoom of the protocol.
const zooms = [1, 4, 16, 64];
const focus = { x: 0.37, y: 0.83 };
const side = check ? 256 : 1024;
/** @param {Uint32Array} p */
const sha = (p) => createHash("sha256").update(new Uint8Array(p.buffer)).digest("hex");
/** @type {{ zoom: number, hashes: Record<string, string> }[]} */
const identity = [];
for (const z of zooms) {
  /** @type {Record<string, string>} */
  const hashes = {};
  for (const [name, render] of Object.entries(renders))
    hashes[name] = sha(render(side, side, z, focus.x, focus.y));
  identity.push({ zoom: z, hashes });
  const values = new Set(Object.values(hashes));
  if (values.size !== 1)
    throw new Error(
      `zoom ${String(z)}: the versions give different pixels: ${JSON.stringify(hashes)}`,
    );
}
const versions = Object.keys(renders).filter((v) => v !== "peras-debug");
if (check) {
  console.log(
    `The garden gives the same pixels in ${versions.join(", ")}, at zooms ${zooms.join(", ")}.`,
  );
  process.exit(0);
}

// 3. Bytes shipped.
const gz = (/** @type {string | Uint8Array} */ x) => gzipSync(x, { level: 9 }).length;
const minified = async (/** @type {string} */ file) =>
  gz((await minify(file, readFileSync(file, "utf8"))).code);
/** @type {Record<string, number>} */
const shipped = {
  typescript: await minified(join(work, "ts", "garden.js")),
  peras:
    (await minified(join(work, "peras", "garden.js"))) +
    gz(readFileSync(join(work, "peras", "garden.wasm"))),
};
if (withRust)
  shipped.rust =
    (await minified(join(work, "rust", "bench_garden_rust.js"))) +
    gz(readFileSync(join(work, "rust", "bench_garden_rust_bg.wasm")));
const lines = (/** @type {string} */ s) =>
  s.split("\n").filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;
/** @type {Record<string, number>} */
const code = {
  typescript: lines(tsSource),
  peras: lines(readFileSync(join(dir, "garden.peras"), "utf8")),
};
if (withRust) code.rust = lines(readFileSync(join(dir, "rust", "src", "lib.rs"), "utf8"));

// 4. Images: the procedural render as the reference, stored images against it.
// Peras's render, the same bytes as the others.
const ref = /** @type {Render} */ (renders.peras);
/** Writes a render as a PNG; gives its path. @param {string} name @param {Uint32Array} pixels */
function png(name, pixels) {
  const raw = join(work, "img", `${name}.rgba`);
  writeFileSync(raw, new Uint8Array(pixels.buffer));
  const out = join(work, "img", `${name}.png`);
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
/** SSIM and PSNR of `image`, decoded and scaled to `size`, against `reference`, scaled the same. */
function quality(/** @type {string} */ image, /** @type {string} */ reference, size = side) {
  const scale = `scale=${String(size)}:${String(size)}:flags=area,format=yuv444p`;
  const r = sh("ffmpeg", [
    ...["-hide_banner", "-i", image, "-i", reference],
    "-lavfi",
    `[0:v]${scale}[a];[1:v]${scale}[b];[a][b]ssim;[0:v]${scale}[c];[1:v]${scale}[d];[c][d]psnr`,
    ...["-f", "null", "-"],
  ]);
  const ssim = Number(/SSIM .*All:([0-9.]+)/.exec(r.stderr)?.[1] ?? "0");
  const psnr = Number(
    /PSNR .*average:([0-9.inf]+)/.exec(r.stderr)?.[1]?.replace("inf", "99") ?? "0",
  );
  return { ssim, psnr };
}
/** An AVIF of `source` at `crf`, in `chroma`; gives its path. */
function avif(
  /** @type {string} */ source,
  /** @type {number} */ crf,
  /** @type {string} */ chroma,
) {
  const out = source.replace(/\.png$/, `.${chroma}.${String(crf)}.avif`);
  sh("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    "-i",
    source,
    "-c:v",
    "libaom-av1",
    "-still-picture",
    "1",
    "-crf",
    String(crf),
    "-cpu-used",
    "6",
    "-pix_fmt",
    chroma,
    out,
  ]);
  return out;
}
function webp(/** @type {string} */ source, /** @type {number} */ q) {
  const out = source.replace(/\.png$/, `.${String(q)}.webp`);
  sh("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    "-i",
    source,
    "-c:v",
    "libwebp",
    "-quality",
    String(q),
    "-compression_level",
    "6",
    out,
  ]);
  return out;
}
const bytes = (/** @type {string} */ file) => statSync(file).size;
/**
 * The smallest AVIF of `source` in `chroma` reaching an SSIM of `target`,
 * searched over the CRF, which orders the files by size; a size of
 * `Infinity` when none reaches it, not even at the best quality.
 * @returns {{ bytes: number, ssim: number, crf: number }}
 */
function smallestIn(/** @type {string} */ source, /** @type {string} */ chroma, target = 0.95) {
  let lo = 0;
  let hi = 63;
  /** @type {{ bytes: number, ssim: number, crf: number }} */
  let best = { bytes: Infinity, ssim: 0, crf: -1 };
  while (lo <= hi) {
    const crf = Math.floor((lo + hi) / 2);
    const file = avif(source, crf, chroma);
    const { ssim } = quality(file, source);
    if (ssim >= target) {
      if (bytes(file) < best.bytes) best = { bytes: bytes(file), ssim, crf };
      lo = crf + 1;
    } else hi = crf - 1;
  }
  return best;
}
const chromas = ["yuv444p", "yuv420p"];
/**
 * The smallest AVIF of `source` reaching an SSIM of 0.95, in either chroma
 * subsampling: what the protocol calls the smallest AVIF.
 * @returns {{ chroma: string, bytes: number, ssim: number, crf: number, reached: boolean }}
 */
function smallestAvif(/** @type {string} */ source) {
  const found = chromas.map((chroma) => ({ chroma, ...smallestIn(source, chroma) }));
  const best = found.reduce((a, b) => (b.bytes < a.bytes ? b : a));
  return { ...best, reached: Number.isFinite(best.bytes) };
}

// H1: the whole scene, at zoom 1.
const whole = png("whole", ref(side, side, 1, 0.5, 0.5));
const h1Avif = chromas.map((chroma) => ({ chroma, ...smallestIn(whole, chroma) }));
const h1Best = smallestAvif(whole);
const lossless = bytes(whole);

// H2: tiles of the scene at each zoom, a fixed random sample.
let seed = 13;
const random = () => {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed / 4294967296;
};
const tilesPerZoom = 8;
/** @type {{ zoom: number, tiles: number, reached: number, meanBytes: number, minBytes: number, maxBytes: number, wholeScene: number | null, chromas: string }[]} */
const h2 = [];
for (const z of zooms) {
  /** @type {number[]} */
  const sizes = [];
  /** @type {string[]} */
  const used = [];
  const count = z === 1 ? 1 : tilesPerZoom;
  for (let t = 0; t < count; t++) {
    const half = 0.5 / z;
    const x = z === 1 ? 0.5 : half + random() * (1 - 2 * half);
    const y = z === 1 ? 0.5 : half + random() * (1 - 2 * half);
    const tile = z === 1 ? whole : png(`tile-${String(z)}-${String(t)}`, ref(side, side, z, x, y));
    const best = smallestAvif(tile);
    if (best.reached) sizes.push(best.bytes);
    used.push(best.reached ? best.chroma : "none");
  }
  const mean = sizes.length > 0 ? sizes.reduce((a, b) => a + b, 0) / sizes.length : 0;
  h2.push({
    zoom: z,
    tiles: count,
    reached: sizes.length,
    meanBytes: mean,
    minBytes: sizes.length > 0 ? Math.min(...sizes) : 0,
    maxBytes: sizes.length > 0 ? Math.max(...sizes) : 0,
    // Estimated only when every tile drawn reached the target.
    wholeScene: sizes.length === count ? mean * z * z : null,
    chromas: used.join(" "),
  });
}

// H4: from afar, the same files reduced 4 times.
/** @type {{ format: string, setting: string, bytes: number, full: number, far: number }[]} */
const h4 = [];
for (const [format, settings] of /** @type {const} */ ([
  ["webp", [20, 50, 80]],
  ["avif", [50, 35, 20]],
])) {
  for (const s of settings) {
    const file = format === "webp" ? webp(whole, s) : avif(whole, s, h1Best.chroma);
    h4.push({
      format,
      setting: format === "webp" ? `quality ${String(s)}` : `crf ${String(s)}`,
      bytes: bytes(file),
      full: quality(file, whole).ssim,
      far: quality(file, whole, side / 4).ssim,
    });
  }
}

// 5. Time in Chromium, and the memory each holds.
const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".html": "text/html",
});
const page = `<!doctype html><meta charset="utf-8"><title>garden</title><link rel="icon" href="data:,">`;
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  if (path === "/") {
    response.writeHead(200, { "content-type": "text/html" });
    response.end(page);
    return;
  }
  try {
    const file = join(work, path);
    response.writeHead(200, { "content-type": types[extname(file)] ?? "text/plain" });
    response.end(readFileSync(file));
  } catch {
    response.writeHead(404);
    response.end();
  }
});
await new Promise((resolve) => {
  server.listen(0, () => {
    resolve(undefined);
  });
});
const address = server.address();
const port = typeof address === "object" && address !== null ? address.port : 0;
const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
const tab = await browser.newPage();
await tab.goto(`http://localhost:${String(port)}/`);
/** @type {unknown} */
const timed = await tab.evaluate(
  `(async () => {
    const side = ${String(side)};
    const t = await import("/ts/garden.js");
    const p = await import("/peras/garden.js");
    const d = await import("/debug/garden.js");
    const versions = {
      typescript: () => { const px = new Uint32Array(side * side); return () => t.render(px, side, side, 1, 0.5, 0.5); },
      peras: () => { const a = p.__perasHeap.array("i32", side * side); return () => p.render(a, side, side, 1, 0.5, 0.5); },
      "peras-debug": () => { const a = d.__perasHeap.array("i32", side * side); return () => d.render(a, side, side, 1, 0.5, 0.5); },
    };
    if (${String(withRust)}) {
      const r = await import("/rust/bench_garden_rust.js");
      await r.default("/rust/bench_garden_rust_bg.wasm");
      versions.rust = () => { const px = new r.Pixels(side * side); return () => r.render(px, side, side, 1, 0.5, 0.5); };
    }
    const calls = Object.fromEntries(Object.entries(versions).map(([k, make]) => [k, make()]));
    const times = Object.fromEntries(Object.keys(calls).map((k) => [k, []]));
    for (let run = 0; run < 17; run++)
      for (const [k, call] of Object.entries(calls)) {
        const t0 = performance.now();
        call();
        const ms = performance.now() - t0;
        if (run >= 2) times[k].push(ms);
      }
    return times;
  })()`,
);
await browser.close();
server.close();
const times = /** @type {Record<string, number[]>} */ (timed);
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;
/** @type {Record<string, number>} */
const ms = Object.fromEntries(Object.entries(times).map(([k, v]) => [k, median(v)]));

// 6. The verdicts, by the thresholds of the protocol.
const tsMs = ms.typescript ?? 0;
const perasMs = ms.peras ?? 1;
/** @typedef {{ verdict: "holds" | "refuted" | "undetermined", detail: string }} Verdict */
const verdict = (/** @type {boolean} */ holds) => (holds ? "holds" : "refuted");
/** @type {Record<string, Verdict>} */
const verdicts = {
  H1: {
    verdict: verdict((shipped.peras ?? Infinity) < h1Best.bytes),
    detail: `Peras ships ${String(shipped.peras)} bytes; the smallest AVIF at SSIM 0.95 weighs ${String(h1Best.bytes)} (${h1Best.chroma}, crf ${String(h1Best.crf)})`,
  },
  H2: (() => {
    const first = h2[0]?.wholeScene ?? null;
    const last = h2.at(-1)?.wholeScene ?? null;
    const missing = h2
      .filter((r) => r.reached < r.tiles)
      .map(
        (r) => `zoom ${String(r.zoom)}: ${String(r.tiles - r.reached)} of ${String(r.tiles)} tiles`,
      );
    if (first === null || last === null)
      return {
        verdict: "undetermined",
        detail: `no AVIF reached SSIM 0.95 for some tiles (${missing.join("; ")}), so the bytes of the whole scene cannot be estimated`,
      };
    return {
      verdict: verdict(last / first >= 16),
      detail: `the AVIF of the whole scene at zoom 64 is ${(last / first).toFixed(1)} times the one at zoom 1${missing.length > 0 ? `; unreached: ${missing.join("; ")}` : ""}`,
    };
  })(),
  H3: (() => {
    const vsTs = tsMs / perasMs;
    const vsRust = ms.rust === undefined ? undefined : ms.rust / perasMs;
    return {
      verdict: verdict(vsTs >= 1.3 && (vsRust === undefined || vsRust >= 0.85)),
      detail: `Peras at ${vsTs.toFixed(2)} times TypeScript's speed${vsRust === undefined ? ", Rust not built" : `, ${vsRust.toFixed(2)} times Rust's`}`,
    };
  })(),
  H4: {
    verdict: verdict(h4.every((r) => r.far > r.full)),
    detail: h4
      .map((r) => `${r.format} ${r.setting}: ${r.full.toFixed(3)} → ${r.far.toFixed(3)}`)
      .join("; "),
  },
  H5: {
    verdict: verdict((ms["peras-debug"] ?? Infinity) <= 1.5 * perasMs),
    detail: `debug at ${((ms["peras-debug"] ?? 0) / perasMs).toFixed(2)} times production's time`,
  },
};

const results = {
  machine: cpus()[0]?.model ?? "?",
  side,
  identity,
  shipped,
  code,
  ms,
  lossless,
  h1Avif,
  h2,
  h4,
  verdicts,
};
writeFileSync(join(dir, "results.json"), `${JSON.stringify(results, null, 2)}\n`);
const kb = (/** @type {number} */ n) => `${(n / 1024).toFixed(1)} KiB`;
const report = [
  "# The garden of Apeiron — results",
  "",
  `Generated by \`node scripts/bench-garden.mjs\` under the protocol of [decision 0013](../../docs/en/decisions/0013-garden-of-apeiron.md), registered before these measures. Machine: ${results.machine}. Renders of ${String(side)} × ${String(side)} pixels. Every number is also in [results.json](results.json).`,
  "",
  "## Verdicts",
  "",
  "| Hypothesis | Verdict | Measured |",
  "| --- | --- | --- |",
  ...Object.entries(verdicts).map(
    ([h, v]) => `| ${h} | ${v.verdict === "holds" ? "holds" : `**${v.verdict}**`} | ${v.detail} |`,
  ),
  "",
  "## Departures from the protocol",
  "",
  "- The [first run](run-1/RESULTS.md) searched each tile's AVIF in one chroma subsampling, 4:2:0, the smaller at zoom 1; for zoomed tiles no AVIF in 4:2:0 reached SSIM 0.95, and the script counted the missing size as H2 holding. The method was corrected: each tile's smallest AVIF is searched in 4:4:4 and 4:2:0, as \"the smallest AVIF\" of the protocol says, and a tile no AVIF brings to 0.95 is counted as unreached, making H2 undetermined rather than holding. The hypotheses and thresholds did not change.",
  "",
  "## Same pixels",
  "",
  `SHA-256 of each render, centred on (${String(focus.x)}, ${String(focus.y)}): ${versions.join(", ")} and Peras built for debug give the same bytes at every zoom.`,
  "",
  "| Zoom | SHA-256 |",
  "| --: | --- |",
  ...identity.map(
    (r) => `| ${String(r.zoom)} | \`${Object.values(r.hashes)[0]?.slice(0, 16) ?? ""}…\` |`,
  ),
  "",
  "## The program and its images",
  "",
  "| | Bytes, minified and gzip | Lines of code |",
  "| --- | --: | --: |",
  ...Object.keys(shipped).map((k) => `| ${k} | ${String(shipped[k])} | ${String(code[k] ?? "")} |`),
  `| the render as PNG, lossless | ${String(lossless)} | |`,
  ...h1Avif.map(
    (a) =>
      `| the render as AVIF ${a.chroma}, smallest at SSIM ≥ 0.95 | ${Number.isFinite(a.bytes) ? `${String(a.bytes)} (crf ${String(a.crf)}, SSIM ${a.ssim.toFixed(3)})` : "not reached"} | |`,
  ),
  "",
  "## Zoomed: what a stored image must spend",
  "",
  `The whole scene at zoom _z_ is _z_² tiles of ${String(side)} × ${String(side)}; the bytes are estimated from ${String(tilesPerZoom)} tiles drawn at random (one at zoom 1, the whole scene), each the smallest AVIF, 4:4:4 or 4:2:0, reaching SSIM 0.95 against its procedural render. A tile no AVIF brings to 0.95 is unreached. The program stays at ${kb(shipped.peras ?? 0)}.`,
  "",
  "| Zoom | Tiles reached | AVIF per tile, mean (min – max) | Whole scene | Against zoom 1 | Chroma per tile |",
  "| --: | --: | --: | --: | --: | --- |",
  ...h2.map(
    (r) =>
      `| ${String(r.zoom)} | ${String(r.reached)} of ${String(r.tiles)} | ${r.reached > 0 ? `${kb(r.meanBytes)} (${kb(r.minBytes)} – ${kb(r.maxBytes)})` : "–"} | ${r.wholeScene === null ? "not estimated" : kb(r.wholeScene)} | ${r.wholeScene === null || h2[0]?.wholeScene == null ? "–" : `${(r.wholeScene / h2[0].wholeScene).toFixed(1)} ×`} | ${r.chromas} |`,
  ),
  "",
  "## From afar",
  "",
  "The same files, then the same files and the reference reduced 4 times by area, as the eye integrates them from farther.",
  "",
  "| Format | Setting | Bytes | SSIM, full size | SSIM, reduced 4 times |",
  "| --- | --- | --: | --: | --: |",
  ...h4.map(
    (r) =>
      `| ${r.format} | ${r.setting} | ${String(r.bytes)} | ${r.full.toFixed(4)} | ${r.far.toFixed(4)} |`,
  ),
  "",
  "## Speed",
  "",
  `Milliseconds per ${String(side)} × ${String(side)} frame of the whole scene, in Chromium, medians of 15 runs after 2 of warm-up, the versions alternating.`,
  "",
  "| Version | ms | Against TypeScript |",
  "| --- | --: | --: |",
  ...Object.entries(ms).map(([k, v]) => `| ${k} | ${v.toFixed(1)} | ${(tsMs / v).toFixed(2)} |`),
  "",
];
writeFileSync(join(dir, "RESULTS.md"), report.join("\n"));
console.log(report.join("\n"));
