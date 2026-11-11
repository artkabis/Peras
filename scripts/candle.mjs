// The candle and the garden: the same zoom into the garden of Apeiron, drawn
// by three versions, each with a candle whose wax burns as the processor works.
//   A. TypeScript as often written (bench/candle/garden-objects.ts): a new
//      object for each colour, neighbouring cell and petal.
//   B. TypeScript by hand (bench/garden/garden.ts), the garden's reference.
//   C. Peras (bench/garden/garden.peras), compiled for production.
// 1. Same pixels: SHA-256 of every frame of every version, all equal.
// 2. Time: milliseconds of each frame, in Chromium, in 5 rounds, the versions
//    in a new order each round; a frame's time is the median of the rounds.
//    The JavaScript heap after each frame, where Chromium gives it.
// 3. The animation, drawn from those numbers only: the wax a candle has lost
//    is its version's time so far, the flame its time for this frame.
// The processor's time is what is measured; it drives the energy spent, but
// this machine has no counter of energy, and the animation says so.
// Usage: node scripts/candle.mjs [--check | --draw]
//        --check: the three versions give the same pixels on a few frames,
//        in Node; nothing else is measured or written.
//        --draw: draws the animation again from bench/candle/results.json,
//        without measuring.
// Writes bench/candle/results.json, bench/candle/RESULTS.md and
// docs/assets/candle.gif and candle.fr.gif, unless --check.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpus } from "node:os";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { Buffer } from "node:buffer";
import { pathToFileURL } from "node:url";
import { chromium } from "playwright";
import ts from "typescript";

const root = join(import.meta.dirname, "..");
const check = process.argv.includes("--check");
const drawOnly = process.argv.includes("--draw");
const work = join(root, "target", "candle");
const commit = spawnSync("git", ["rev-parse", "--short=7", "HEAD"], {
  cwd: root,
  encoding: "utf8",
}).stdout.trim();
rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "js"), { recursive: true });
mkdirSync(join(work, "frames"), { recursive: true });

/** @param {string} command @param {string[]} args */
function sh(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 1 << 28 });
  if (result.status !== 0) throw new Error(`${command} ${args.join(" ")}\n${result.stderr}`);
  return result;
}

// The versions.
for (const [from, to] of [
  ["bench/candle/garden-objects.ts", "objects.js"],
  ["bench/garden/garden.ts", "hand.js"],
]) {
  const js = ts.transpileModule(readFileSync(join(root, from), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  writeFileSync(join(work, "js", to), js);
}
sh(join(root, "target", "release", "perasc"), [
  "build",
  join(root, "bench", "garden", "garden.peras"),
  "--allocator",
  "tlsf",
  "--out",
  join(work, "peras"),
]);

// The zoom: from the whole garden to a flower of the meadow, 64 times.
const FRAMES = 40;
const SIDE = 200;
const ROUNDS = 5;
const focus = { x: 0.37, y: 0.83 };
const views = Array.from({ length: FRAMES }, (_, i) => {
  const zoom = 64 ** (i / (FRAMES - 1));
  return { zoom, cx: focus.x + (0.5 - focus.x) / zoom, cy: focus.y + (0.5 - focus.y) / zoom };
});

if (check) {
  /** @typedef {(p: Uint32Array, w: number, h: number, z: number, x: number, y: number) => number} Render */
  /** @typedef {{ render: (p: unknown, w: number, h: number, z: number, x: number, y: number) => number, __perasHeap: { array: (k: string, n: number) => { view: () => Int32Array, release: () => void } } }} Peras */
  /** @param {string} path @returns {Promise<unknown>} */
  const load = async (path) => {
    /** @type {unknown} */
    const loaded = await import(pathToFileURL(path).href);
    return loaded;
  };
  const objects = /** @type {{ render: Render }} */ (await load(join(work, "js", "objects.js")));
  const hand = /** @type {{ render: Render }} */ (await load(join(work, "js", "hand.js")));
  const peras = /** @type {Peras} */ (await load(join(work, "peras", "garden.js")));
  const side = 64;
  const sha = (/** @type {Uint8Array} */ bytes) => createHash("sha256").update(bytes).digest("hex");
  for (const i of [0, Math.floor(FRAMES / 2), FRAMES - 1]) {
    const v = views[i] ?? { zoom: 1, cx: 0.5, cy: 0.5 };
    const a = new Uint32Array(side * side);
    const b = new Uint32Array(side * side);
    objects.render(a, side, side, v.zoom, v.cx, v.cy);
    hand.render(b, side, side, v.zoom, v.cx, v.cy);
    const c = peras.__perasHeap.array("i32", side * side);
    peras.render(c, side, side, v.zoom, v.cx, v.cy);
    const pc = new Uint8Array(c.view().slice().buffer);
    c.release();
    const hashes = [sha(new Uint8Array(a.buffer)), sha(new Uint8Array(b.buffer)), sha(pc)];
    if (new Set(hashes).size !== 1) {
      console.error(`frame ${String(i)}: the versions give different pixels`, hashes);
      process.exit(1);
    }
  }
  console.log(
    "The candle's three versions give the same pixels on its first, middle and last frames.",
  );
  process.exit(0);
}

// Measure in Chromium.
const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
});
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  if (path === "/") {
    response.writeHead(200, { "content-type": "text/html" });
    response.end(
      '<!doctype html><meta charset="utf-8"><title>candle</title><link rel="icon" href="data:,">',
    );
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
const browser = await chromium.launch({
  ...(executablePath === undefined ? {} : { executablePath }),
  args: ["--enable-precise-memory-info"],
});
const page = await browser.newPage({ viewport: { width: 900, height: 600 }, deviceScaleFactor: 1 });
await page.goto(`http://localhost:${String(port)}/`);

const setup = `
  const SIDE = ${String(SIDE)};
  const views = ${JSON.stringify(views)};
  const objects = await import("/js/objects.js");
  const hand = await import("/js/hand.js");
  const peras = await import("/peras/garden.js");
  const perasPixels = peras.__perasHeap.array("i32", SIDE * SIDE);
  const draw = {
    objects: (v) => { const p = new Uint32Array(SIDE * SIDE); objects.render(p, SIDE, SIDE, v.zoom, v.cx, v.cy); return p; },
    hand: (v) => { const p = new Uint32Array(SIDE * SIDE); hand.render(p, SIDE, SIDE, v.zoom, v.cx, v.cy); return p; },
    peras: (v) => { peras.render(perasPixels, SIDE, SIDE, v.zoom, v.cx, v.cy); return null; },
  };
  const pixelsOf = (name, v) => name === "peras" ? new Uint32Array(perasPixels.view().slice().buffer) : draw[name](v);
`;

/** @typedef {{ commit: string, frameMs: Record<string, number[]>, totals: Record<string, number>, heapSpread: Record<string, number> }} Measured */
/** Measures in Chromium; refuses if the versions give different pixels. @returns {Promise<Measured>} */
async function measure() {
  /** @type {unknown} */
  const measured = await page.evaluate(
    `(async () => {
      ${setup}
      const names = Object.keys(draw);
      const hex = async (p) => [...new Uint8Array(await crypto.subtle.digest("SHA-256", p.buffer))].map((b) => b.toString(16).padStart(2, "0")).join("");
      // Same pixels, frame by frame, before any time is taken.
      const same = [];
      for (const v of views) {
        const hashes = [];
        for (const name of names) {
          if (name === "peras") { draw.peras(v); hashes.push(await hex(pixelsOf("peras", v))); }
          else hashes.push(await hex(draw[name](v)));
        }
        same.push(new Set(hashes).size === 1);
      }
      const times = Object.fromEntries(names.map((n) => [n, views.map(() => [])]));
      const heap = Object.fromEntries(names.map((n) => [n, []]));
      for (let round = 0; round < ${String(ROUNDS)}; round++) {
        const order = names.map((_, i) => names[(i + round) % names.length]);
        for (const name of order) {
          for (let i = 0; i < views.length; i++) {
            const t0 = performance.now();
            draw[name](views[i]);
            times[name][i].push(performance.now() - t0);
            heap[name].push(performance.memory ? performance.memory.usedJSHeapSize : 0);
          }
        }
      }
      return { same, times, heap };
    })()`,
  );
  const { same, times, heap } =
    /** @type {{ same: boolean[], times: Record<string, number[][]>, heap: Record<string, number[]> }} */ (
      measured
    );
  if (!same.every(Boolean)) throw new Error("the versions give different pixels; nothing is drawn");
  /** @param {number[]} xs */
  const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;
  /** @type {Record<string, number[]>} */
  const frameMs = Object.fromEntries(
    Object.entries(times).map(([name, frames]) => [name, frames.map((f) => median(f))]),
  );
  const totals = Object.fromEntries(
    Object.entries(frameMs).map(([name, f]) => [name, f.reduce((a, b) => a + b, 0)]),
  );
  const heapSpread = Object.fromEntries(
    Object.entries(heap).map(([name, h]) => [name, Math.max(...h) - Math.min(...h)]),
  );
  return { commit, frameMs, totals, heapSpread };
}

/** The numbers of the last measure, to draw again (`--draw`). @returns {Measured} */
function saved() {
  /** @type {unknown} */
  const parsed = JSON.parse(readFileSync(join(root, "bench", "candle", "results.json"), "utf8"));
  const json =
    /** @type {{ commit: string, same: boolean, frameMs: Record<string, number[]>, totalMs: Record<string, number>, heapSpreadBytes: Record<string, number> }} */ (
      parsed
    );
  if (!json.same)
    throw new Error("the saved measure did not find the same pixels; nothing is drawn");
  return {
    commit: json.commit,
    frameMs: json.frameMs,
    totals: json.totalMs,
    heapSpread: json.heapSpreadBytes,
  };
}

// The commit the numbers were measured at: kept when they are drawn again.
const { commit: measuredAt, frameMs, totals, heapSpread } = drawOnly ? saved() : await measure();
writeResults();

// Draw the animation from the numbers, in English and in French.
const words = {
  en: {
    title: "The candle and the garden",
    zoom: "zoom",
    names: ["TypeScript, as often written", "TypeScript, by hand", "Peras"],
    frame: "this frame",
    total: "so far",
    note: "Wax: processor time measured for each frame, median of 5 rounds, Chromium. Same pixels in all three, checked.",
    end: "Processor time for the whole zoom",
    whole: "the whole zoom",
  },
  fr: {
    title: "La bougie et le jardin",
    zoom: "zoom",
    names: ["TypeScript, comme on l’écrit souvent", "TypeScript, à la main", "Peras"],
    frame: "cette image",
    total: "jusqu’ici",
    note: "Cire : temps de calcul mesuré pour chaque image, médiane de 5 tours, Chromium. Mêmes pixels dans les trois, vérifiés.",
    end: "Temps de calcul pour tout le zoom",
    whole: "tout le zoom",
  },
};
const HOLD = 24;
for (const [lang, w] of Object.entries(words)) {
  const dir = join(work, "frames", lang);
  mkdirSync(dir, { recursive: true });
  /** @type {unknown} */
  const pngs = await page.evaluate(
    `(async () => {
      ${setup}
      const w = ${JSON.stringify(w)};
      const lang = ${JSON.stringify(lang)};
      const frameMs = ${JSON.stringify(frameMs)};
      const totals = ${JSON.stringify(totals)};
      const names = ["objects", "hand", "peras"];
      const most = Math.max(...Object.values(totals));
      const longest = Math.max(...names.flatMap((n) => frameMs[n]));
      const GAP = 24, HEAD = 52, LABEL = 40, WAX = 120, FOOT = 88;
      const W = GAP + names.length * (SIDE + GAP), H = HEAD + SIDE + LABEL + 70 + WAX + FOOT;
      const canvas = document.createElement("canvas");
      canvas.width = W; canvas.height = H;
      const ctx = canvas.getContext("2d");
      const fmt = new Intl.NumberFormat(lang === "fr" ? "fr-FR" : "en-US", { maximumFractionDigits: 1, minimumFractionDigits: 1 });
      const sec = (ms) => fmt.format(ms / 1000) + " s";
      const msText = (ms) => new Intl.NumberFormat(lang === "fr" ? "fr-FR" : "en-US", { maximumFractionDigits: 0 }).format(ms) + " ms";
      const image = new ImageData(SIDE, SIDE);
      const out = [];
      const total = views.length + ${String(HOLD)};
      for (let f = 0; f < total; f++) {
        const i = Math.min(f, views.length - 1);
        const end = f >= views.length;
        const v = views[i];
        ctx.fillStyle = "#14130f"; ctx.fillRect(0, 0, W, H);
        ctx.fillStyle = "#efe7d4"; ctx.font = "600 20px DejaVu Sans, sans-serif";
        ctx.fillText(w.title, GAP, 32);
        ctx.fillStyle = "#a59d88"; ctx.font = "14px DejaVu Sans, sans-serif";
        const z = w.zoom + " ×" + new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(v.zoom);
        ctx.fillText(z, W - GAP - ctx.measureText(z).width, 32);
        names.forEach((name, k) => {
          const x0 = GAP + k * (SIDE + GAP);
          draw[name](v);
          image.data.set(new Uint8ClampedArray(pixelsOf(name, v).buffer));
          ctx.putImageData(image, x0, HEAD);
          ctx.fillStyle = name === "peras" ? "#efe7d4" : "#cfc6b0"; ctx.font = (name === "peras" ? "600 " : "") + "13px DejaVu Sans, sans-serif";
          // A name wider than its panel goes on two lines, cut at a space.
          const label = w.names[k];
          if (ctx.measureText(label).width <= SIDE) ctx.fillText(label, x0, HEAD + SIDE + 18);
          else {
            const at = label.lastIndexOf(" ", Math.ceil(label.length / 2) + 4);
            ctx.fillText(label.slice(0, at), x0, HEAD + SIDE + 18);
            ctx.fillText(label.slice(at + 1), x0, HEAD + SIDE + 34);
          }
          // The candle: its wax is the time not yet spent, its flame this frame's.
          const spent = frameMs[name].slice(0, i + 1).reduce((a, b) => a + b, 0);
          const left = 1 - 0.92 * (spent / most);
          const cx = x0 + SIDE / 2, base = HEAD + SIDE + LABEL + 70 + WAX;
          const top = base - WAX * left;
          const grd = ctx.createLinearGradient(cx - 22, 0, cx + 22, 0);
          grd.addColorStop(0, "#d9cdb0"); grd.addColorStop(0.5, "#f4ecd8"); grd.addColorStop(1, "#cbbf9f");
          ctx.fillStyle = grd; ctx.fillRect(cx - 22, top, 44, base - top);
          ctx.fillStyle = "#2b2820"; ctx.fillRect(cx - 34, base, 68, 6);
          ctx.strokeStyle = "#3a352a"; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(cx, top); ctx.lineTo(cx, top - 7); ctx.stroke();
          const heat = end ? 0.25 : Math.max(0.25, frameMs[name][i] / longest);
          const fh = 14 + 46 * heat, fw = 7 + 9 * heat;
          const glow = ctx.createRadialGradient(cx, top - 7 - fh * 0.45, 2, cx, top - 7 - fh * 0.45, fh * 1.6);
          glow.addColorStop(0, "rgba(255,190,90,0.35)"); glow.addColorStop(1, "rgba(255,190,90,0)");
          ctx.fillStyle = glow; ctx.beginPath(); ctx.arc(cx, top - 7 - fh * 0.45, fh * 1.6, 0, Math.PI * 2); ctx.fill();
          ctx.fillStyle = "#ffcf6e"; ctx.beginPath();
          ctx.moveTo(cx, top - 7 - fh);
          ctx.bezierCurveTo(cx + fw, top - 7 - fh * 0.45, cx + fw * 0.8, top - 5, cx, top - 5);
          ctx.bezierCurveTo(cx - fw * 0.8, top - 5, cx - fw, top - 7 - fh * 0.45, cx, top - 7 - fh);
          ctx.fill();
          ctx.fillStyle = "#fff2c9"; ctx.beginPath(); ctx.ellipse(cx, top - 7 - fh * 0.3, fw * 0.35, fh * 0.28, 0, 0, Math.PI * 2); ctx.fill();
          ctx.font = "12px DejaVu Sans, sans-serif"; ctx.fillStyle = "#a59d88";
          const line1 = end ? w.whole : w.frame + " " + msText(frameMs[name][i]);
          const line2 = (end ? "" : w.total + " ") + sec(spent);
          ctx.fillText(line1, x0, base + 22);
          ctx.fillStyle = name === "peras" ? "#efe7d4" : "#cfc6b0"; ctx.font = (end ? "600 18px" : "13px") + " DejaVu Sans, sans-serif";
          ctx.fillText(line2, x0, base + (end ? 46 : 42));
        });
        ctx.fillStyle = "#857e6c"; ctx.font = "11px DejaVu Sans, sans-serif";
        ctx.fillText(w.note, GAP, H - 10);
        out.push(canvas.toDataURL("image/png"));
      }
      return out;
    })()`,
  );
  /** @type {string[]} */
  const list = /** @type {string[]} */ (pngs);
  list.forEach((png, n) => {
    writeFileSync(
      join(dir, `f${String(n).padStart(3, "0")}.png`),
      Buffer.from(png.split(",")[1] ?? "", "base64"),
    );
  });
  const out = join(root, "docs", "assets", lang === "en" ? "candle.gif" : "candle.fr.gif");
  sh("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    "-framerate",
    "8",
    "-i",
    join(dir, "f%03d.png"),
    "-vf",
    "split[a][b];[a]palettegen=max_colors=160:stats_mode=full[p];[b][p]paletteuse=dither=bayer:bayer_scale=4",
    "-loop",
    "0",
    out,
  ]);
}
await browser.close();
server.close();

function writeResults() {
  const results = {
    commit: measuredAt,
    machine: cpus()[0]?.model ?? "unknown",
    frames: FRAMES,
    side: SIDE,
    rounds: ROUNDS,
    views,
    same: true,
    frameMs,
    totalMs: totals,
    heapSpreadBytes: heapSpread,
  };
  mkdirSync(join(root, "bench", "candle"), { recursive: true });
  writeFileSync(
    join(root, "bench", "candle", "results.json"),
    `${JSON.stringify(results, null, 2)}\n`,
  );
  const f1 = (/** @type {number} */ x) => x.toFixed(1);
  const names = {
    objects: "A. TypeScript, as often written",
    hand: "B. TypeScript, by hand",
    peras: "C. Peras",
  };
  const report = [
    "# The candle and the garden — results",
    "",
    `Generated by \`node scripts/candle.mjs\` at commit \`${measuredAt}\`, on ${results.machine}. The same zoom into the garden of Apeiron, ${String(FRAMES)} frames of ${String(SIDE)} × ${String(SIDE)} pixels from zoom 1 to 64, drawn by three versions that give the same pixels, checked by SHA-256 frame by frame. Every number is also in [results.json](results.json).`,
    "",
    "| Version | Processor time for the zoom | Against C |",
    "| --- | --: | --: |",
    ...Object.entries(names).map(
      ([key, name]) =>
        `| ${name} | ${f1(totals[key] ?? 0)} ms | ${((totals[key] ?? 0) / (totals.peras ?? 1)).toFixed(2)} × |`,
    ),
    "",
    `A frame's time is the median of ${String(ROUNDS)} rounds, the versions in a new order each round, in Chromium; the zoom's time is the sum of its frames'. What is measured is the processor's time, which drives the energy spent; this machine has no counter of energy, and the animation says what it shows. No memory is reported: the three versions run in one page and share one JavaScript heap, where what one leaves is collected while another runs, so its size belongs to none of them.`,
    "",
  ];
  writeFileSync(join(root, "bench", "candle", "RESULTS.md"), report.join("\n"));
  console.log(report.join("\n"));
}
