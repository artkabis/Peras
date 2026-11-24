// Decision 0043's protocol: arenas. Each option is a prototype the compiler
// builds when PERAS_PROTO_ARENA is `a1` (`arena { … }` blocks written, the
// versions of bench/arena/a1), `a2` (arenas chosen by the compiler) or `a3`
// (more sizes kept by TLSF); without it, a0, today.
//
// The workloads, owned modules built for production (`--allocator tlsf`),
// each option's module and a0's loaded in one process and checked to give
// the same result, then run the order alternating round by round (finding
// M017): the median time, its ratio to a0 round by round, the CPU time over
// the wall time, the module's memory after the workload, the blocks left
// alive, the bytes of the module, raw and gzip.
//
// Usage: node scripts/bench-arena.mjs [--check] [--rounds N]
//        --check: one round, the results checked, nothing written.
// Writes bench/arena/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { basename, join } from "node:path";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";
import ts from "typescript";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "arena");
const work = join(root, "target", "bench-arena");
const perasc = join(root, "target", "release", "perasc");
/** @type {Record<string, string>} */
const OPTIONS = {
  a0: "a0, today",
  a1: "a1, `arena { … }` written",
  a1b: "a1b, a1 grown as needed, on for a block",
  a2: "a2, chosen by the compiler",
  a3: "a3, more sizes kept",
};
const options = Object.keys(OPTIONS);

rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "js"), { recursive: true });

/**
 * Builds a source with an option.
 * @param {string} source
 * @param {string} option
 */
function build(source, option) {
  const dir = join(work, `${basename(source, ".peras")}-${option}`);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const env = { ...process.env, PERAS_PROTO_ARENA: option === "a0" ? "" : option };
  const args = ["build", source, "--out", dir, "--color", "never", "--allocator", "tlsf"];
  const r = spawnSync(perasc, args, { encoding: "utf8", env });
  if (r.status !== 0) throw new Error(`${source}, ${option}: ${r.stderr}`);
  const name = basename(source, ".peras");
  // The module's memory, read by the measure only.
  const text = readFileSync(join(dir, `${name}.js`), "utf8").replace(
    "export const __perasHeap = {",
    "export const __perasHeap = {\n  memory: () => $runtime.memory.buffer.byteLength,",
  );
  const measured = join(dir, `${name}.measure.mjs`);
  writeFileSync(measured, text);
  const wasm = readFileSync(join(dir, `${name}.wasm`));
  return { js: measured, bytes: { raw: wasm.length, gzip: gzipSync(wasm).length } };
}

/** @typedef {Record<string, (...args: unknown[]) => unknown> & { __perasHeap?: { live(): number, memory(): number } }} Loaded */
/** @param {string} js @returns {Promise<Loaded>} */
async function load(js) {
  /** @type {unknown} */
  const loaded = await import(pathToFileURL(js).href);
  return /** @type {Loaded} */ (loaded);
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;

// W1's data: the generator of decision 0014, transpiled as written.
for (const name of ["generate", "summary"]) {
  const source = readFileSync(join(root, "bench", "data", `${name}.ts`), "utf8");
  const js = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText;
  writeFileSync(join(work, "js", `${name}.js`), js.replaceAll(/\.ts(["'])/g, ".js$1"));
}
const generator = /** @type {{ generate: (rows: number) => Uint8Array }} */ (
  /** @type {unknown} */ (await load(join(work, "js", "generate.js")))
);
const summary = /** @type {{ canonical: (s: unknown) => string }} */ (
  /** @type {unknown} */ (await load(join(work, "js", "summary.js")))
);
const rows = check ? 20_000 : 1_000_000;
const many = check ? 10_000 : 1_000_000;
const frames = check ? 5 : 100;
const count = (/** @type {number} */ n) => n.toLocaleString("en");
const csv = generator.generate(rows);

/** @type {[string, string, string, unknown[]][]} */
const workloads = [
  [
    `W1, the dashboard, ${rows.toLocaleString("en")} rows`,
    join(root, "bench", "data", "dashboard.peras"),
    "summarize",
    [csv],
  ],
  [
    `W2, the fireflies, a frame's temporaries: 10,000 fireflies, ${count(frames)} frames`,
    join(bench, "frame.peras"),
    "run",
    [10_000, frames],
  ],
  [`W3, a line of text a row, ${count(many)} rows`, join(bench, "rows.peras"), "run", [many]],
  [
    `W4, values that leave their call, ${count(many)} strings`,
    join(bench, "keep.peras"),
    "run",
    [many],
  ],
  [
    `W4b, values kept for a whole call, ${count(many)} strings`,
    join(bench, "keep-call.peras"),
    "run",
    [many],
  ],
];
const same = (/** @type {unknown} */ x) =>
  typeof x === "object" && x !== null ? summary.canonical(x) : String(x);

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; owned modules on TLSF, perasc built for release.`,
  "",
];
for (const [label, source, fn, args] of workloads) {
  /** @type {Record<string, { m: Loaded, bytes: { raw: number, gzip: number } }>} */
  const modules = {};
  for (const o of options) {
    const own = join(bench, "a1", basename(source));
    const built = build(o.startsWith("a1") && existsSync(own) ? own : source, o);
    modules[o] = { m: await load(built.js), bytes: built.bytes };
  }
  const want = same(modules.a0?.m[fn]?.(...args));
  for (const o of options) {
    const got = same(modules[o]?.m[fn]?.(...args));
    if (got !== want)
      throw new Error(`${label}, ${o}: ${got.slice(0, 80)}, not ${want.slice(0, 80)}`);
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
    "| Option | Median | / a0, median (range) | CPU / wall | Memory | Left alive | Bytes (gzip) |",
  );
  lines.push("| --- | ---: | --- | ---: | ---: | ---: | ---: |");
  for (const o of options) {
    const own = times[o] ?? [];
    const ratios = own.map((t, i) => t / (times.a0?.[i] ?? Number.NaN));
    const heap = modules[o]?.m.__perasHeap;
    const bytes = modules[o]?.bytes;
    lines.push(
      `| ${OPTIONS[o] ?? o} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[o] ?? 0) / (wall[o] ?? 1)).toFixed(2)} | ${heap ? `${String(heap.memory() / 1024)} KiB` : "-"} | ${heap ? String(heap.live()) : "-"} | ${String(bytes?.raw)} (${String(bytes?.gzip)}) |`,
    );
    if (heap && heap.live() !== 0)
      throw new Error(`${label}, ${o}: ${String(heap.live())} blocks left alive`);
  }
  lines.push("");
}

console.log(lines.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# Arenas: the options measured\n\nGenerated by \`node scripts/bench-arena.mjs\`, for [decision 0043](../../docs/en/decisions/0043-arenas.md).\n\n${lines.join("\n")}`,
  );
}
