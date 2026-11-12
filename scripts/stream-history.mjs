// The runs of decision 0015, side by side: how many times faster Peras (C) is
// than TypeScript as written (A) and TypeScript written for speed (B), at
// 100,000 rows, each with its 95 % interval, run after run, as the compiler
// changed under the same protocol. Read from each run's results.json; no
// number is typed here.
// Usage: node scripts/stream-history.mjs
// Writes docs/assets/stream-history.svg.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "stream");

/** @typedef {{ value: number, ci: number[] }} Estimate */
/** @typedef {{ commit: string, bySize: Record<string, { speedupCoverA: Estimate, speedupCoverB: Estimate, versions: Record<string, { nsPerRow: number }> }> }} Results */

/** What changed before each run, in order. */
const changes = [
  "as registered",
  "static literals",
  "runtime linked",
  "fallback, block cache",
  "one lookup",
  "SIMD split",
];

/**
 * @param {string} file
 * @returns {Results}
 */
function load(file) {
  const parsed = /** @type {unknown} */ (JSON.parse(readFileSync(file, "utf8")));
  return /** @type {Results} */ (parsed);
}

/** @type {{ label: string, results: Results }[]} */
const runs = [];
for (let n = 1; ; n++) {
  const file = join(dir, `run-${String(n)}`, "results.json");
  if (!existsSync(file)) break;
  runs.push({
    label: changes[n - 1] ?? `run ${String(n)}`,
    results: load(file),
  });
}
const current = join(dir, "results.json");
if (existsSync(current))
  runs.push({
    label: changes[runs.length] ?? `run ${String(runs.length + 1)}`,
    results: load(current),
  });

const size = "100000";
const n0 = (/** @type {number} */ x) => String(Math.round(x * 10) / 10);
const W = 720;
const plot = { x: 70, y: 70, w: W - 150, h: 230 };
/** @param {Results} r @param {"speedupCoverA" | "speedupCoverB"} key */
const estimate = (r, key) => r.bySize[size]?.[key] ?? { value: 0, ci: [0, 0] };
const top = Math.ceil(
  Math.max(...runs.flatMap((run) => [estimate(run.results, "speedupCoverA").ci[1] ?? 0])) + 0.5,
);
const x = (/** @type {number} */ i) =>
  plot.x + (runs.length <= 1 ? plot.w / 2 : (plot.w * i) / (runs.length - 1));
const y = (/** @type {number} */ v) => plot.y + plot.h * (1 - v / top);

const style = `<style>
  .bg{fill:#fcfcfb}.ink{fill:#0b0b0b}.muted{fill:#52514e}.grid{stroke:#e6e5df}.even{stroke:#9a9890;stroke-dasharray:4 3}
  .A{fill:#eb6834;stroke:#eb6834}.B{fill:#1baf7a;stroke:#1baf7a}
  text{font-family:system-ui,sans-serif;font-size:12px;font-variant-numeric:tabular-nums}
  .title{font-size:14px;font-weight:600}
  .val{paint-order:stroke;stroke:#fcfcfb;stroke-width:4px;stroke-linejoin:round}
  @media (prefers-color-scheme: dark){
    .bg{fill:#1a1a19}.ink{fill:#f0efec}.muted{fill:#c3c2b7}.grid{stroke:#2c2c2a}.even{stroke:#6b6a63}.val{stroke:#1a1a19}
    .A{fill:#d95926;stroke:#d95926}.B{fill:#199e70;stroke:#199e70}
  }
</style>`;
let body = `<text class="ink title" x="20" y="28">How many times faster Peras is, run after run, at 100,000 rows</text>`;
body += `<rect class="A" x="20" y="43" width="12" height="12" rx="2"/><text class="ink" x="38" y="53">over TypeScript as written</text>`;
body += `<rect class="B" x="230" y="43" width="12" height="12" rx="2"/><text class="ink" x="248" y="53">over TypeScript written for speed</text>`;
for (let t = 0; t <= top; t++)
  body += `<line class="grid" x1="${n0(plot.x)}" x2="${n0(plot.x + plot.w)}" y1="${n0(y(t))}" y2="${n0(y(t))}"/><text class="muted" x="${n0(plot.x - 8)}" y="${n0(y(t) + 4)}" text-anchor="end">${String(t)}×</text>`;
body += `<line class="even" x1="${n0(plot.x)}" x2="${n0(plot.x + plot.w)}" y1="${n0(y(1))}" y2="${n0(y(1))}"/><text class="muted" x="${n0(plot.x + plot.w)}" y="${n0(y(1) - 6)}" text-anchor="end">as fast</text>`;
for (const [key, cls] of /** @type {const} */ ([
  ["speedupCoverA", "A"],
  ["speedupCoverB", "B"],
])) {
  const points = runs.map((run, i) => ({ i, e: estimate(run.results, key) }));
  body += `<path class="${cls}" style="fill:none" stroke-width="2" d="${points.map((p, k) => `${k === 0 ? "M" : "L"}${n0(x(p.i))},${n0(y(p.e.value))}`).join("")}"/>`;
  for (const p of points) {
    body += `<line class="${cls}" stroke-width="2" x1="${n0(x(p.i))}" x2="${n0(x(p.i))}" y1="${n0(y(p.e.ci[0] ?? 0))}" y2="${n0(y(p.e.ci[1] ?? 0))}"/>`;
    body += `<circle class="${cls}" cx="${n0(x(p.i))}" cy="${n0(y(p.e.value))}" r="4.5"/>`;
    body += `<text class="ink val" x="${n0(x(p.i) + 7)}" y="${n0(y(p.e.value) - 6)}">${p.e.value.toFixed(2)}</text>`;
  }
}
runs.forEach((run, i) => {
  body += `<text class="ink" x="${n0(x(i))}" y="${n0(plot.y + plot.h + 18)}" text-anchor="middle">run ${String(i + 1)}</text>`;
  body += `<text class="muted" x="${n0(x(i))}" y="${n0(plot.y + plot.h + 34)}" text-anchor="middle">${run.label}</text>`;
});
const first = runs[0]?.results.bySize[size]?.versions.C?.nsPerRow ?? 0;
const last = runs[runs.length - 1]?.results.bySize[size]?.versions.C?.nsPerRow ?? 0;
const note = `Same protocol, data and TypeScript in every run; only the compiler changed. Dot: geometric mean over 10 browser processes; line: 95 % interval. Peras's time per row: ${String(Math.round(first))} ns in run 1, ${String(Math.round(last))} ns in run ${String(runs.length)}. Decision 0015, Chromium.`;
const lines = [""];
for (const word of note.split(" ")) {
  const k = lines.length - 1;
  if ((lines[k] ?? "").length + word.length > 105) lines.push(word);
  else lines[k] = lines[k] === "" ? word : `${lines[k] ?? ""} ${word}`;
}
const noteTop = plot.y + plot.h + 64;
const total = noteTop + lines.length * 16 + 4;
body += lines
  .map((l, i) => `<text class="muted" x="20" y="${String(noteTop + i * 16)}">${l}</text>`)
  .join("");
writeFileSync(
  join(root, "docs", "assets", "stream-history.svg"),
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${String(W)} ${String(total)}" width="${String(W)}" height="${String(total)}" role="img" aria-label="How many times faster Peras is, run after run">${style}<rect class="bg" width="${String(W)}" height="${String(total)}" rx="8"/>${body}</svg>\n`,
);
console.log(`Written: docs/assets/stream-history.svg, ${String(runs.length)} runs.`);
