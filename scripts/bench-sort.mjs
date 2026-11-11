// Sorting benchmark: `sortBy` and `sortByDescending` of bench/sort/kernels.apeiron,
// compiled to JavaScript and, as a `.peras` file, to WebAssembly with the TLSF
// allocator, against what TypeScript writes, `sort((a, b) => a.amount - b.amount)`,
// in Node (V8, the engine of Chrome). Each side makes the same records from the
// same random sequence, sorts them and folds their order into a checksum; the
// three checksums must agree. The time of making the records alone is
// subtracted: what remains is the sort's. Medians of the runs after two of warm-up.
// Usage: node scripts/bench-sort.mjs [--check] [--perasc <path>]
//        --check: two sizes, three runs: the sides agree, and the managed
//        sort by a number is not slower than TypeScript's at 10,000 records.
// Writes bench/sort/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const check = process.argv.includes("--check");
const at = process.argv.indexOf("--perasc");
const root = join(import.meta.dirname, "..");
const perasc = at > 0 ? (process.argv[at + 1] ?? "") : join(root, "target", "release", "perasc");
const work = join(root, "target", "bench-sort");
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });
const source = join(root, "bench", "sort", "kernels.apeiron");
copyFileSync(source, join(work, "owned.peras"));
/** @param {string[]} args */
const build = (args) => {
  const built = spawnSync(perasc, args, { encoding: "utf8" });
  if (built.status !== 0)
    throw new Error(`perasc ${args.join(" ")}\n${built.stdout}${built.stderr}`);
};
build(["build", source, "--out", join(work, "managed")]);
build(["build", join(work, "owned.peras"), "--allocator", "tlsf", "--out", join(work, "owned")]);

/** @typedef {{ made: (n: number) => number, byAmount: (n: number) => number, byAmountDown: (n: number) => number, byName: (n: number) => number }} Kernels */
/** @type {unknown} */
const managedModule = await import(pathToFileURL(join(work, "managed", "kernels.js")).href);
/** @type {unknown} */
const ownedModule = await import(pathToFileURL(join(work, "owned", "owned.js")).href);
const managed = /** @type {Kernels} */ (managedModule);
const owned = /** @type {Kernels & { __perasHeap: { checkLeaks(): void } }} */ (ownedModule);

/** @typedef {{ id: number, amount: number, name: string }} Rec */
/** @param {number} n @returns {Rec[]} */
const records = (n) => {
  /** @type {Rec[]} */
  const xs = [];
  let s = 17;
  for (let i = 0; i < n; i++) {
    s = (Math.imul(s, 1103515245) + 12345) | 0;
    const r = Math.abs(Math.trunc(s / 65536) % 100000);
    xs.push({ id: i, amount: r * 0.01, name: `c${String(r % 5000)}` });
  }
  return xs;
};
/** @param {Rec[]} xs */
const checksum = (xs) => xs.reduce((c, x) => (Math.imul(c, 31) + x.id) | 0, 0);
/** What TypeScript writes. @type {Kernels} */
const ts = {
  made: (n) => checksum(records(n)),
  byAmount: (n) => checksum(records(n).sort((a, b) => a.amount - b.amount)),
  byAmountDown: (n) => checksum(records(n).sort((a, b) => b.amount - a.amount)),
  byName: (n) =>
    checksum(records(n).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))),
};

const sides = /** @type {const} */ ([
  ["ts", ts],
  ["managed", managed],
  ["owned", owned],
]);
const sizes = check ? [300, 10000] : [1000, 10000, 100000];
const runs = check ? 3 : 15;
/** @param {number[]} values */
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ?? 0;
/** @param {(n: number) => number} f @param {number} n */
const time = (f, n) => {
  const t0 = performance.now();
  f(n);
  return performance.now() - t0;
};
const kinds = /** @type {const} */ ([
  ["byAmount", "by a number, increasing"],
  ["byAmountDown", "by a number, decreasing"],
  ["byName", "by a string"],
]);
const lines = [];
let managedAt10k = 0;
for (const n of sizes) {
  for (const [kind, label] of kinds) {
    const results = sides.map(([, f]) => f[kind](n));
    if (results.some((r) => r !== results[0]))
      throw new Error(
        `${kind}(${String(n)}): TypeScript, managed, owned give ${results.join(" | ")}`,
      );
    /** @type {Record<string, number[]>} */
    const times = { ts: [], managed: [], owned: [] };
    for (let r = 0; r < runs + 2; r++) {
      for (const [side, f] of sides) {
        const sorted = time(f[kind], n);
        const made = time(f.made, n);
        if (r >= 2) times[side]?.push(Math.max(sorted - made, 0.001));
      }
    }
    const [t, m, o] = ["ts", "managed", "owned"].map((side) => median(times[side] ?? []));
    const speed = (/** @type {number} */ x) => ((t ?? 0) / x).toFixed(2);
    if (n === 10000 && kind === "byAmount") managedAt10k = (t ?? 0) / (m ?? 1);
    lines.push(
      `| ${String(n)} | ${label} | ${(t ?? 0).toFixed(2)} | ${(m ?? 0).toFixed(2)} (${speed(m ?? 1)}) | ${(o ?? 0).toFixed(2)} (${speed(o ?? 1)}) |`,
    );
  }
}
owned.__perasHeap.checkLeaks();
if (check) {
  if (managedAt10k < 1)
    throw new Error(
      `sortBy by a number, managed, runs at ${managedAt10k.toFixed(2)} times TypeScript's speed at 10,000 records`,
    );
  console.log(
    "sortBy gives what TypeScript's sort gives, in both modes, with no leak, and is not slower by a number.",
  );
} else {
  const report = [
    "# Sorting — results",
    "",
    `Generated by \`node scripts/bench-sort.mjs\`, in Node ${process.version} (V8). Machine: ${cpus()[0]?.model ?? "?"}. Milliseconds of the sort alone, medians of ${String(runs)} runs after two of warm-up; speed: TypeScript's time divided by Peras's. TypeScript is \`sort((a, b) => a.amount - b.amount)\`; Peras is \`sortBy((x) => x.amount)\`, in a managed module (JavaScript) and an owned one (WebAssembly, TLSF allocator). The three give the same order, checked first.`,
    "",
    "| Records | Sorted | TypeScript (ms) | Peras managed (ms, speed) | Peras owned (ms, speed) |",
    "| --: | --- | --: | --: | --: |",
    ...lines,
    "",
  ].join("\n");
  writeFileSync(join(root, "bench", "sort", "RESULTS.md"), report);
  console.log(report);
}
