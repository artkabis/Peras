// The demo: the same animation of particles in three versions, written in
// TypeScript with immutable objects as it is often written, in TypeScript
// with typed arrays as an expert writes it for speed, and in Peras compiled
// to WebAssembly (bench/demo). Each runs alone in Chromium for the same time,
// with the same particles, drawing on its canvas its frames per second and
// the memory the page holds; each is recorded, then the three are put side by
// side with ffmpeg, as a video and a GIF.
// Usage: node scripts/demo.mjs [--check] [--n <particles>] [--seconds <s>]
//        --check: two seconds each, no video: the three run and draw.
// Writes bench/demo/RESULTS.md and docs/assets/demo.gif, and the whole video
// as target/demo/demo.mp4, unless --check.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { Buffer } from "node:buffer";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { chromium } from "playwright";
import ts from "typescript";

const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "demo");
const check = process.argv.includes("--check");
/** @param {string} name @param {number} fallback */
const option = (name, fallback) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? Number(process.argv[at + 1]) : fallback;
};
const n = option("--n", check ? 200000 : 1500000);
const seconds = check ? 2 : option("--seconds", 10);
const perasc = join(root, "target", "release", "perasc");
const work = join(root, "target", "demo");
rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "ts"), { recursive: true });
for (const f of ["index.html", "page.js"]) copyFileSync(join(dir, f), join(work, f));
for (const name of ["particles-immutable", "particles-typed"]) {
  const source = readFileSync(join(dir, `${name}.ts`), "utf8");
  const js = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  writeFileSync(join(work, "ts", `${name}.js`), js);
}
const built = spawnSync(
  perasc,
  ["build", join(dir, "particles.peras"), "--out", join(work, "peras"), "--allocator", "tlsf"],
  { encoding: "utf8" },
);
if (built.status !== 0) throw new Error(`perasc build\n${built.stderr}`);

/** @type {Record<string, string>} */
const types = { ".js": "text/javascript", ".html": "text/html", ".wasm": "application/wasm" };
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  try {
    const file = join(work, path === "/" ? "index.html" : path);
    response.writeHead(200, {
      "content-type": types[extname(file)] ?? "text/plain",
    });
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
  // The heap as it is, not rounded: the curve each version draws.
  args: ["--enable-precise-memory-info"],
});

/** @typedef {{ frames: { at: number, gap: number, spent: number }[], memory: number[] }} Recorded */
const versions = /** @type {const} */ (["immutable", "typed", "peras"]);
/** @type {Record<string, { video: string, recorded: Recorded }>} */
const runs = {};
for (const v of versions) {
  const videoDir = join(work, "video", v);
  mkdirSync(videoDir, { recursive: true });
  const context = await browser.newContext({ viewport: { width: 480, height: 360 } });
  const tab = await context.newPage();
  await tab.goto(`http://localhost:${String(port)}/?v=${v}&n=${String(n)}`);
  await tab.waitForFunction(() => "demo" in globalThis);
  // The frames the browser paints, from DevTools' screencast, each with the
  // time it was painted; turned into a video at a steady rate below.
  /** @type {{ at: number, data: string }[]} */
  const shots = [];
  const cdp = await context.newCDPSession(tab);
  if (!check) {
    cdp.on("Page.screencastFrame", (frame) => {
      shots.push({ at: frame.metadata.timestamp ?? 0, data: frame.data });
      void cdp.send("Page.screencastFrameAck", { sessionId: frame.sessionId });
    });
    await cdp.send("Page.startScreencast", { format: "jpeg", quality: 85 });
  }
  await tab.waitForTimeout(seconds * 1000);
  if (!check) await cdp.send("Page.stopScreencast");
  // At 25 frames a second, each slot shows the last frame painted before it.
  const rate = 25;
  const first = shots[0]?.at ?? 0;
  let k = 0;
  for (let slot = 0; slot < seconds * rate; slot++) {
    const t = first + slot / rate;
    while (k + 1 < shots.length && (shots[k + 1]?.at ?? Infinity) <= t) k++;
    const shot = shots[k];
    if (shot !== undefined)
      writeFileSync(
        join(videoDir, `${String(slot).padStart(5, "0")}.jpg`),
        Buffer.from(shot.data, "base64"),
      );
  }
  const recorded = /** @type {Recorded} */ (
    await tab.evaluate(
      () => /** @type {{ demo: unknown }} */ (/** @type {unknown} */ (globalThis)).demo,
    )
  );
  await context.close();
  runs[v] = { video: join(videoDir, "%05d.jpg"), recorded };
}
await browser.close();
server.close();

/** @param {number[]} xs @param {number} q */
const quantile = (xs, q) => [...xs].sort((a, b) => a - b)[Math.floor((xs.length - 1) * q)] ?? 0;
const rows = versions.map((v) => {
  const { frames, memory } = runs[v]?.recorded ?? { frames: [], memory: [] };
  // The first second is the engine's warm-up.
  const start = (frames[0]?.at ?? 0) + 1000;
  const kept = frames.filter((f) => f.at >= start);
  const spanned = ((kept.at(-1)?.at ?? 0) - (kept[0]?.at ?? 0)) / 1000;
  const fps = spanned > 0 ? (kept.length - 1) / spanned : 0;
  const spent = kept.map((f) => f.spent);
  const late = kept.filter((f) => f.gap > 1000 / 60 + 4).length;
  const mem = memory.slice(frames.length - kept.length).map((b) => b / 1048576);
  return {
    v,
    fps,
    median: quantile(spent, 0.5),
    p95: quantile(spent, 0.95),
    late: kept.length > 0 ? (100 * late) / kept.length : 0,
    low: Math.min(...mem),
    high: Math.max(...mem),
  };
});
const names = {
  immutable: "TypeScript, immutable objects",
  typed: "TypeScript, typed arrays by hand",
  peras: "Peras, WebAssembly",
};
const table = [
  "| Version | Frames per second | Frame computed in (median, 95th percentile) | Frames late | Memory held, lowest to highest |",
  "| --- | --: | --: | --: | --: |",
  ...rows.map(
    (r) =>
      `| ${names[r.v]} | ${r.fps.toFixed(1)} | ${r.median.toFixed(1)} ms, ${r.p95.toFixed(1)} ms | ${r.late.toFixed(0)} % | ${r.low.toFixed(1)} to ${r.high.toFixed(1)} MiB |`,
  ),
].join("\n");
console.log(table);
if (check) {
  if (rows.some((r) => r.fps < 1)) throw new Error("a version of the demo did not run");
  console.log("The three versions of the demo run and draw.");
} else {
  mkdirSync(join(root, "docs", "assets"), { recursive: true });
  const [a, b, c] = versions.map((v) => runs[v]?.video ?? "");
  const input = (/** @type {string} */ pattern) => ["-framerate", "25", "-i", pattern];
  const mp4 = join(work, "demo.mp4");
  const gif = join(root, "docs", "assets", "demo.gif");
  const ffmpeg = (/** @type {string[]} */ args) => {
    const r = spawnSync("ffmpeg", ["-y", "-loglevel", "error", ...args], { encoding: "utf8" });
    if (r.status !== 0) throw new Error(`ffmpeg ${args.join(" ")}\n${r.stderr}`);
  };
  // The first second of each, the page loading, is cut.
  const cut = `trim=start=1:duration=${String(seconds - 1)},setpts=PTS-STARTPTS`;
  ffmpeg([
    ...input(a),
    ...input(b),
    ...input(c),
    "-filter_complex",
    `[0:v]${cut}[a];[1:v]${cut}[b];[2:v]${cut}[c];[a][b][c]hstack=inputs=3`,
    ...["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "28", "-movflags", "+faststart", mp4],
  ]);
  // The GIF at the top of the README: 6 s, 8 frames a second, 16 colours,
  // about 1 MB; the video keeps everything.
  ffmpeg([
    ...["-t", "6", "-i", mp4],
    "-vf",
    "fps=8,scale=720:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=16:stats_mode=diff[p];[s1][p]paletteuse=dither=none:diff_mode=rectangle",
    gif,
  ]);
  const report = [
    "# Demo — results",
    "",
    `Generated by \`node scripts/demo.mjs\`: ${n.toLocaleString("en")} particles, ${String(seconds)} s each, alone in headless Chromium, the first second left out. Machine: ${cpus()[0]?.model ?? "?"}. The same page runs the three versions; only the code of a frame differs ([page](page.js)). A frame is late when it comes more than 4 ms after the 60 Hz it aims at; a headless browser paints at 60 Hz at most. The memory is the JavaScript heap, plus, for Peras, the module's memory.`,
    "",
    "What it shows and what it does not. The gap between the two TypeScript versions is the garbage collector's: a new object per particle at each frame, as immutable state is often written, collected again and again. The gap between typed arrays and Peras is WebAssembly's, an owned module (`.peras`) with its loops compiled ahead of time; a managed module (`.apeiron`) compiles to JavaScript and runs at TypeScript's speed ([case study](../case-study/RESULTS.md)). A program in TypeScript written as the middle one gets most of the way.",
    "",
    table,
    "",
  ].join("\n");
  writeFileSync(join(dir, "RESULTS.md"), report);
  console.log(`Wrote ${mp4} and ${gif}.`);
}
