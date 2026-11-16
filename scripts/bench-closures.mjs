// Decision 0027's protocol: what a closure does with a string it captures,
// option by option, each a prototype the compiler builds when
// PERAS_PROTO_CLOSURE is `move` (A), `copy` (B) or `lend` (D), and the option
// found by measuring them, `auto` (F): copied, unless the closure cannot
// escape, then lent.
//
// 1. The workloads of bench/closures/workloads, built by each option and as a
//    managed module (JavaScript), checked to give the same result, then run in
//    one process, the order alternating round by round: the median time, its
//    ratio to option B round by round, the CPU time over the wall time, the
//    module's memory after the workload, the blocks left alive, the bytes.
// 2. The five programs of bench/closures/ease, written as a user would: the
//    errors each option meets on them, then the fixed version of each,
//    checked against the program built as a managed module.
//
// Usage: node scripts/bench-closures.mjs [--check] [--rounds N]
//        --check: one round, the results checked, nothing written.
// Writes bench/closures/RESULTS.md, unless --check.
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { basename, join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "closures");
const work = join(root, "target", "bench-closures");
const perasc = join(root, "target", "release", "perasc");
const OPTIONS = {
  move: "A, move",
  copy: "B, copy",
  lend: "D, lend",
  auto: "F, copy unless lent",
  js: "managed",
};

rmSync(work, { recursive: true, force: true });

/**
 * Builds a source with an option, or as a managed module.
 * @param {string} source
 * @param {string} option
 * @returns {{ ok: true, js: string, bytes: { raw: number, gzip: number } } | { ok: false, errors: string[] }}
 */
function build(source, option) {
  const dir = join(work, `${basename(source, ".peras")}-${option}`);
  mkdirSync(dir, { recursive: true });
  let file = source;
  if (option === "js") {
    file = join(dir, `${basename(source, ".peras")}.apeiron`);
    copyFileSync(source, file);
  }
  /** @type {Record<string, string | undefined>} */
  const env = { ...process.env, PERAS_PROTO_CLOSURE: option === "js" ? "" : option };
  const args = ["build", file, "--out", dir, "--color", "never"];
  if (option !== "js") args.push("--allocator", "tlsf");
  const r = spawnSync(perasc, args, { encoding: "utf8", env });
  if (r.status !== 0) {
    const errors = r.stderr.split("\n").filter((l) => l.startsWith("error["));
    return { ok: false, errors };
  }
  const name = basename(file).replace(/\.(peras|apeiron)$/, "");
  const js = join(dir, `${name}.js`);
  // The module's memory, read by the measure only.
  const text = readFileSync(js, "utf8").replace(
    "export const __perasHeap = {",
    "export const __perasHeap = {\n  memory: () => $runtime.memory.buffer.byteLength,",
  );
  const measured = join(dir, `${name}.measure.mjs`);
  writeFileSync(measured, text);
  const shipped = option === "js" ? Buffer.from(text) : readFileSync(join(dir, `${name}.wasm`));
  return {
    ok: true,
    js: measured,
    bytes: { raw: shipped.length, gzip: gzipSync(shipped).length },
  };
}

/** @typedef {Record<string, (...args: unknown[]) => unknown> & { __perasHeap?: { live(): number, memory(): number } }} Loaded */

let loads = 0;
/**
 * A new instance of a built module.
 * @param {string} js
 * @returns {Promise<Loaded>}
 */
async function load(js) {
  loads += 1;
  /** @type {unknown} */
  const loaded = await import(`${js}?load=${String(loads)}`);
  return /** @type {Loaded} */ (loaded);
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;

const w = (/** @type {string} */ f) => join(bench, "workloads", f);
/** @type {[string, string, number[], Record<string, string>][]} */
const workloads = [
  [
    "W1, 16 B, 1,000,000 times",
    "w1",
    [16, 1_000_000],
    { move: "w1-move.peras", copy: "w1.peras", lend: "w1.peras", auto: "w1.peras", js: "w1.peras" },
  ],
  [
    "W1, 1 KiB, 100,000 times",
    "w1",
    [1024, 100_000],
    { move: "w1-move.peras", copy: "w1.peras", lend: "w1.peras", auto: "w1.peras", js: "w1.peras" },
  ],
  [
    "W1, 64 KiB, 2,000 times",
    "w1",
    [65_536, 2000],
    { move: "w1-move.peras", copy: "w1.peras", lend: "w1.peras", auto: "w1.peras", js: "w1.peras" },
  ],
  [
    "W2, 20,000,000 calls",
    "w2",
    [20_000_000],
    { move: "w2.peras", copy: "w2.peras", lend: "w2.peras", auto: "w2.peras", js: "w2.peras" },
  ],
  [
    "W3, 100,000 strings",
    "w3",
    [100_000],
    { move: "w3-move.peras", copy: "w3.peras", auto: "w3.peras", js: "w3.peras" },
  ],
];

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; owned modules on TLSF.`,
  "",
];
for (const [label, fn, args, sources] of workloads) {
  const options = Object.keys(sources);
  /** @type {Record<string, { m: Loaded, bytes: { raw: number, gzip: number } }>} */
  const modules = {};
  for (const o of options) {
    const built = build(w(sources[o] ?? ""), o);
    if (!built.ok) throw new Error(`${label}, ${o}: ${built.errors.join(" | ")}`);
    modules[o] = { m: await load(built.js), bytes: built.bytes };
  }
  // Checked on a small run first, then on the full one.
  const small = args.map((a, i) => (i === 0 ? a : Math.min(a, 100)));
  for (const run of [small, args]) {
    const want = modules.js?.m[fn]?.(...run);
    for (const o of options) {
      const got = modules[o]?.m[fn]?.(...run);
      if (got !== want) throw new Error(`${label}, ${o}: ${String(got)}, not ${String(want)}`);
    }
  }
  /** @type {Record<string, number[]>} */
  const times = Object.fromEntries(options.map((o) => [o, []]));
  /** @type {Record<string, number>} */
  const cpu = Object.fromEntries(options.map((o) => [o, 0]));
  /** @type {Record<string, number>} */
  const wall = Object.fromEntries(options.map((o) => [o, 0]));
  for (let round = 0; round < ROUNDS; round++) {
    const order = round % 2 === 0 ? options : [...options].reverse();
    for (const o of order) {
      const c0 = process.cpuUsage();
      const t0 = performance.now();
      modules[o]?.m[fn]?.(...args);
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[o]?.push(t);
      wall[o] = (wall[o] ?? 0) + t;
      cpu[o] = (cpu[o] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  lines.push(`### ${label}`, "");
  lines.push(
    "| Option | Median | / B, median (range) | CPU / wall | Memory | Left alive | Bytes (gzip) |",
  );
  lines.push("| --- | ---: | --- | ---: | ---: | ---: | ---: |");
  for (const o of options) {
    const own = times[o] ?? [];
    const ratios = own.map((t, i) => t / (times.copy?.[i] ?? Number.NaN));
    const heap = modules[o]?.m.__perasHeap;
    const bytes = modules[o]?.bytes;
    lines.push(
      `| ${OPTIONS[/** @type {keyof typeof OPTIONS} */ (o)]} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[o] ?? 0) / (wall[o] ?? 1)).toFixed(2)} | ${heap ? `${String(heap.memory() / 1024)} KiB` : "-"} | ${heap ? String(heap.live()) : "-"} | ${String(bytes?.raw)} (${String(bytes?.gzip)}) |`,
    );
    if (heap && heap.live() !== 0)
      throw new Error(`${label}, ${o}: ${String(heap.live())} blocks left alive`);
  }
  if (!sources.lend)
    lines.push(`| ${OPTIONS.lend} | not expressible: the closure escapes | | | | | |`);
  lines.push("");
}

// Ease of writing: the errors met by the program as a user writes it.
/** @type {Record<string, unknown[]>} */
const inputs = { p1: ["w"], p2: ["a cat and the dog"], p3: ["Ann"], p4: [12], p5: ["  word  "] };
lines.push("### Ease of writing", "");
lines.push("| Program | A, move | B, copy | D, lend | F, copy unless lent |");
lines.push("| --- | --- | --- | --- | --- |");
for (const p of Object.keys(inputs)) {
  const source = join(bench, "ease", `${p}.peras`);
  const managed = build(source, "js");
  if (!managed.ok) throw new Error(`${p} managed: ${managed.errors.join(" | ")}`);
  const want = JSON.stringify((await load(managed.js))[p]?.(...(inputs[p] ?? [])));
  const cells = [];
  for (const o of ["move", "copy", "lend", "auto"]) {
    const natural = build(source, o);
    const errors = natural.ok ? [] : natural.errors.map((e) => e.slice(6, 11));
    const fixed = join(bench, "ease", "fixed", `${p}-${o}.peras`);
    const final = existsSync(fixed) ? build(fixed, o) : natural;
    if (!final.ok) throw new Error(`${p} ${o}: the fixed program does not build`);
    const m = await load(final.js);
    const got = JSON.stringify(m[p]?.(...(inputs[p] ?? [])));
    if (got !== want || m.__perasHeap?.live() !== 0)
      throw new Error(`${p} ${o}: ${got}, not ${want}`);
    cells.push(errors.length === 0 ? "builds" : errors.join(", "));
  }
  lines.push(`| ${p} | ${cells.join(" | ")} |`);
}
lines.push("");

console.log(lines.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# Closures over the heap: the options measured\n\nGenerated by \`node scripts/bench-closures.mjs\`, for [decision 0027](../../docs/en/decisions/0027-closures-over-the-heap.md).\n\n${lines.join("\n")}`,
  );
}
