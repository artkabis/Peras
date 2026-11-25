// Decision 0047's P3: the frame to a canvas, in Chromium. P2's plasma, 640 by
// 400, drawn and shown 120 frames a round, three ways in one page:
//   typescript: plasma.ts's Uint8ClampedArray, shown as an ImageData over it;
//   view: bench/frame/canvas-f3.peras drawing into an array the page made in
//         the module's memory (a PerasArray), shown through a view, no copy;
//   copy: the same module giving its array back, which crosses as a copy.
// The time of the drawing and of the showing are measured apart, the order
// alternating round by round; the canvas's pixels are read back after the
// last frame of each and must be the same.
//
// Usage: PERAS_CHROMIUM=… node scripts/bench-canvas.mjs [--check]
//        --check: one round of 4 frames, the pixels checked, nothing written.
// Writes bench/frame/CANVAS.md, unless --check.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { cpus } from "node:os";
import { extname, join } from "node:path";
import { chromium } from "playwright";
import ts from "typescript";

const check = process.argv.includes("--check");
const ROUNDS = check ? 1 : 7;
const FRAMES = check ? 4 : 120;
const root = join(import.meta.dirname, "..");
const work = join(root, "target", "bench-canvas");
const perasc = join(root, "target", "release", "perasc");

rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "peras"), { recursive: true });
const built = spawnSync(
  perasc,
  [
    "build",
    join(root, "bench/frame/canvas-f3.peras"),
    "--out",
    join(work, "peras"),
    "--allocator",
    "tlsf",
    "--color",
    "never",
  ],
  { encoding: "utf8" },
);
if (built.status !== 0) throw new Error(built.stderr);
writeFileSync(
  join(work, "plasma.js"),
  ts.transpileModule(readFileSync(join(root, "bench/frame/plasma.ts"), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText,
);

const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
});
const page = `<!doctype html><meta charset="utf-8"><title>canvas</title><link rel="icon" href="data:,"><canvas width="640" height="400"></canvas>`;
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
/** @typedef {{ draw: number[], show: number[], digest: string }} Timed */
const timed = /** @type {Record<string, Timed>} */ (
  await tab.evaluate(
    `(async () => {
    const w = 640, h = 400, frames = ${String(FRAMES)}, rounds = ${String(ROUNDS)};
    const ctx = document.querySelector("canvas").getContext("2d");
    const t = await import("/plasma.js");
    const p = await import("/peras/canvas-f3.js");
    const plasma = new t.Plasma(w, h);
    const lent = p.__perasHeap.array("u32", w * h);
    const made = {
      typescript: { draw: (f) => plasma.draw(f * 0.05), show: () => ctx.putImageData(new ImageData(plasma.pixels, w, h), 0, 0) },
      view: { draw: (f) => p.draw(lent, w, h, f * 0.05), show: () => { const v = lent.view(); ctx.putImageData(new ImageData(new Uint8ClampedArray(v.buffer, v.byteOffset, v.byteLength), w, h), 0, 0); } },
      copy: (() => { let px = []; return { draw: (f) => { px = p.frame(w, h, f * 0.05); }, show: () => ctx.putImageData(new ImageData(new Uint8ClampedArray(Uint32Array.from(px).buffer), w, h), 0, 0) }; })(),
    };
    const names = Object.keys(made);
    const out = Object.fromEntries(names.map((k) => [k, { draw: [], show: [], digest: "" }]));
    for (let round = 0; round < rounds + 1; round++) {
      const order = round % 2 === 0 ? names : [...names].reverse();
      for (const k of order) {
        let draw = 0, show = 0;
        for (let f = 0; f < frames; f++) {
          const t0 = performance.now();
          made[k].draw(f);
          const t1 = performance.now();
          made[k].show();
          const t2 = performance.now();
          draw += t1 - t0; show += t2 - t1;
        }
        const data = ctx.getImageData(0, 0, w, h).data;
        let sum = 0;
        for (let i = 0; i < data.length; i++) sum = (sum * 31 + data[i]) % 1000000007;
        out[k].digest = String(sum);
        // The first round warms each version up, and is not kept.
        if (round > 0) { out[k].draw.push(draw); out[k].show.push(show); }
      }
    }
    return out;
  })()`,
  )
);
await browser.close();
server.close();

const want = timed.typescript?.digest;
for (const [k, v] of Object.entries(timed)) {
  if (v.digest !== want)
    throw new Error(`${k}: the canvas holds ${v.digest}, TypeScript's ${String(want)}`);
}
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
const version = browser.version();
const out = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Chromium ${version}; ${String(ROUNDS)} rounds of ${String(FRAMES)} frames after one to warm up; the module built for production.`,
  "",
  "| Version | Same canvas | Drawing, ms a frame, median | Showing, ms a frame, median | Showing / TypeScript's |",
  "| --- | --- | ---: | ---: | ---: |",
];
const tsShow = median(timed.typescript?.show ?? []);
for (const [k, v] of Object.entries(timed)) {
  const show = median(v.show);
  out.push(
    `| ${k} | yes | ${(median(v.draw) / FRAMES).toFixed(3)} | ${(show / FRAMES).toFixed(3)} | ${(show / tsShow).toFixed(3)} |`,
  );
}
out.push("");
console.log(out.join("\n"));
if (!check) {
  writeFileSync(
    join(root, "bench", "frame", "CANVAS.md"),
    `# A framebuffer of pixels: the frame to a canvas\n\nGenerated by \`node scripts/bench-canvas.mjs\`, for [decision 0047](../../docs/en/decisions/0047-framebuffer.md)'s P3.\n\n${out.join("\n")}`,
  );
}
