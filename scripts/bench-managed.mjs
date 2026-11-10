// Managed modules benchmark: bench/managed/kernels.apeiron compiled to
// JavaScript, and the same source compiled to WebAssembly as a `.peras` file
// with the TLSF allocator, against the same work written by hand in
// JavaScript, in Node (V8, the engine of Chrome). Each case is checked to give
// the same result on the three sides first; then they alternate, medians of
// the runs after two of warm-up.
// Usage: node scripts/bench-managed.mjs [--check] [--perasc <path>]
//        --check: small inputs, one run: only checks the results agree.
// Writes bench/managed/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const check = process.argv.includes("--check");
const at = process.argv.indexOf("--perasc");
const root = join(import.meta.dirname, "..");
const perasc = at > 0 ? (process.argv[at + 1] ?? "") : join(root, "target", "release", "perasc");
const work = join(root, "target", "bench-managed");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
const source = join(root, "bench", "managed", "kernels.apeiron");
copyFileSync(source, join(work, "owned.peras"));
/** @param {string[]} args */
const run = (args) => {
  const built = spawnSync(perasc, args, { encoding: "utf8" });
  if (built.status !== 0)
    throw new Error(`perasc ${args.join(" ")}\n${built.stdout}${built.stderr}`);
};
run(["build", source, "--out", join(work, "managed")]);
run(["build", join(work, "owned.peras"), "--allocator", "tlsf", "--out", join(work, "owned")]);

/** @typedef {{ sum(xs: number[]): number, step(xs: number[], vs: number[]): void, words(t: string): number, gravity(n: number): number }} Kernels */
/** @type {unknown} */
const managedModule = await import(pathToFileURL(join(work, "managed", "kernels.js")).href);
/** @type {unknown} */
const ownedModule = await import(pathToFileURL(join(work, "owned", "owned.js")).href);
const managed = /** @type {Kernels} */ (managedModule);
const owned = /** @type {Kernels & { __perasHeap: { checkLeaks(): void } }} */ (ownedModule);

/** The same work, written by hand in JavaScript. @type {Kernels} */
const js = {
  sum(xs) {
    let s = 0;
    // An indexed loop, as the fastest JavaScript is written: `for…of` takes
    // half as long again in V8.
    // eslint-disable-next-line @typescript-eslint/prefer-for-of
    for (let i = 0; i < xs.length; i++) s = (s + (xs[i] ?? 0)) | 0;
    return s;
  },
  step(xs, vs) {
    for (let i = 0; i < xs.length; i++) {
      let v = vs[i] ?? 0;
      const x = (xs[i] ?? 0) + v * 0.01;
      if (x < 0 || x > 10) v = -v;
      vs[i] = v;
      xs[i] = Math.sqrt(x) * 0.5 + x * 0.5;
    }
  },
  words(t) {
    /** @type {Map<string, number>} */
    const counts = new Map();
    for (const w of t.split(" ")) counts.set(w, (counts.get(w) ?? 0) + 1);
    return counts.size;
  },
  gravity(n) {
    const bodies = [];
    for (let i = 0; i < n; i++)
      bodies.push({ x: Math.sin(i * 0.37) * 10, y: Math.cos(i * 0.71) * 10, mass: 1 + (i % 3) });
    let energy = 0;
    for (let i = 0; i < bodies.length; i++) {
      for (let j = i + 1; j < bodies.length; j++) {
        const a = bodies[i];
        const b = bodies[j];
        if (a === undefined || b === undefined) continue;
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        energy -= (a.mass * b.mass) / Math.sqrt(dx * dx + dy * dy + 0.01);
      }
    }
    return energy;
  },
};

const scale = check ? 0.01 : 1;
const n = Math.round(1_000_000 * scale);
const ints = Array.from({ length: n }, (_, i) => Math.imul(i, -1640531535));
const xs = Array.from({ length: n }, (_, i) => (i % 1000) * 0.01);
const vs = xs.map((x) => Math.sin(x * 7));
const text = Array.from({ length: n / 2 }, (_, i) => `w${String((i * 7919) % 5000)}`).join(" ");
const bodies = Math.round(2000 * Math.sqrt(scale));

/** @type {[string, (f: Kernels) => unknown][]} */
const cases = [
  [`sum of ${String(n)} i32, wrapping`, (f) => f.sum(ints)],
  [
    `${String(n)} particles, one step, inout arrays`,
    (f) => {
      const [a, b] = [xs.slice(), vs.slice()];
      f.step(a, b);
      return `${String(a[n - 1])} ${String(b[n >> 1])}`;
    },
  ],
  [`count ${String(n / 2)} words, Map<String, i32>`, (f) => f.words(text)],
  [`energy of ${String(bodies)} bodies, structures`, (f) => f.gravity(bodies)],
];
/** @param {number[]} values */
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? 0;
const runs = check ? 1 : 15;
const sides = /** @type {const} */ ([
  ["js", js],
  ["managed", managed],
  ["owned", owned],
]);
const lines = [];
for (const [name, call] of cases) {
  const results = sides.map(([, f]) => String(call(f)));
  if (results.some((r) => r !== results[0]))
    throw new Error(`${name}: JavaScript, managed, owned give ${results.join(" | ")}`);
  /** @type {Record<string, number[]>} */
  const times = { js: [], managed: [], owned: [] };
  for (let r = 0; r < runs + (check ? 0 : 2); r++) {
    for (const [side, f] of sides) {
      const t0 = performance.now();
      call(f);
      if (r >= 2 || check) times[side]?.push(performance.now() - t0);
    }
  }
  const [j, m, o] = ["js", "managed", "owned"].map((side) => median(times[side] ?? []));
  const speed = (/** @type {number} */ t) => ((j ?? 0) / t).toFixed(2);
  lines.push(
    `| ${name} | ${(j ?? 0).toFixed(1)} | ${(m ?? 0).toFixed(1)} | ${speed(m ?? 1)} | ${(o ?? 0).toFixed(1)} | ${speed(o ?? 1)} |`,
  );
}
owned.__perasHeap.checkLeaks();
if (check) {
  console.log("Managed and owned modules give what JavaScript gives, with no leak.");
} else {
  const report = [
    "# Managed modules — results",
    "",
    "Generated by `node scripts/bench-managed.mjs`: [bench/managed/kernels.apeiron](kernels.apeiron) compiled to JavaScript, and the same source compiled to WebAssembly as a `.peras` file with the TLSF allocator, against the same work written by hand in JavaScript, in Node.",
    "",
    `Machine: ${cpus()[0]?.model ?? "?"}, Node ${process.version}. Medians of ${String(runs)} runs after 2 of warm-up, the three alternating. Speed: the time of hand-written JavaScript divided by the module's; above 1 is faster.`,
    "",
    "| Case | JavaScript (ms) | Managed (ms) | Speed | Owned (ms) | Speed |",
    "| --- | --: | --: | --: | --: | --: |",
    ...lines,
    "",
  ].join("\n");
  writeFileSync(join(root, "bench", "managed", "RESULTS.md"), report);
  console.log(report);
}
