// Decision 0044's protocol: pools. Each option is a prototype the compiler
// builds when PERAS_PROTO_POOL is `p1` (Pool<T> built in, generational
// handles), `p2` (plain handles) or `p3` (Pool<T> written in Peras,
// imported); without it, p0, indices by hand. p1 and p2 run the programs of
// bench/pool/p1, p3 those of bench/pool/p3, p0 those of bench/pool.
//
// 1. Each workload built for production by each option, run first in a
//    child process with a time limit, twice on one instance; its result
//    compared with p0's. A module that does not end, or computes another
//    result, is reported as such and not timed.
// 2. The modules that compute p0's result, in one process, the order
//    alternating round by round (finding M017): the median time, its ratio
//    to p0 round by round, the CPU time over the wall time, the module's
//    memory at the end, its bytes, raw and gzip.
// 3. Safety: bench/pool/stale.peras under each option: the stale read
//    caught (-1) or not.
//
// Usage: node scripts/bench-pool.mjs [--check] [--rounds N]
//        --check: small sizes, one round, the results checked, nothing written.
// Writes bench/pool/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "pool");
const work = join(root, "target", "bench-pool");
const perasc = join(root, "target", "release", "perasc");
/** @type {Record<string, string>} */
const OPTIONS = {
  p0: "p0, indices by hand",
  p1: "p1, built in, generational handles",
  p2: "p2, built in, plain handles",
  p3: "p3, written in Peras, imported",
};
const options = Object.keys(OPTIONS);

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/** The source of a program for an option. @param {string} name @param {string} option */
function source(name, option) {
  const own =
    option === "p3" ? join(bench, "p3", `${name}.peras`) : join(bench, "p1", `${name}.peras`);
  return option !== "p0" && existsSync(own) ? own : join(bench, `${name}.peras`);
}

/**
 * Builds a program with an option, for production.
 * @param {string} name @param {string} option
 */
function build(name, option) {
  const dir = join(work, `${name}-${option}`);
  const env = { ...process.env, PERAS_PROTO_POOL: option === "p0" ? "" : option };
  const args = [
    "build",
    source(name, option),
    "--out",
    dir,
    "--color",
    "never",
    "--allocator",
    "tlsf",
  ];
  const r = spawnSync(perasc, args, { encoding: "utf8", env });
  if (r.status !== 0) throw new Error(`${name}, ${option}: ${r.stderr}`);
  const text = readFileSync(join(dir, `${name}.js`), "utf8").replace(
    "export const __perasHeap = {",
    "export const __perasHeap = {\n  memory: () => $runtime.memory.buffer.byteLength,",
  );
  const measured = join(dir, `${name}.measure.mjs`);
  writeFileSync(measured, text);
  const wasm = readFileSync(join(dir, `${name}.wasm`));
  return { js: measured, bytes: { raw: wasm.length, gzip: gzipSync(wasm).length } };
}

/** The result of a module called twice, in a child process, or why none came. @param {string} js @param {number[]} args */
function probe(js, args) {
  const code = `import(${JSON.stringify(pathToFileURL(js).href)}).then((m) => {
    const a = m.run(${args.join(",")}); const b = m.run(${args.join(",")});
    console.log(JSON.stringify([String(a), String(b)]));
  });`;
  const r = spawnSync(process.execPath, ["-e", code], {
    encoding: "utf8",
    timeout: check ? 20_000 : 120_000,
  });
  if (r.status !== 0)
    return {
      error: r.signal === "SIGTERM" ? "does not end" : `fails: ${r.stderr.split("\n")[0] ?? ""}`,
    };
  /** @type {unknown} */
  const parsed = JSON.parse(r.stdout);
  const [a, b] = /** @type {[string, string]} */ (parsed);
  return a === b ? { value: a } : { error: `two calls differ: ${a}, ${b}` };
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
const count = (/** @type {number} */ n) => n.toLocaleString("en");
const frames = check ? 20 : 600;
const nodes = check ? 5000 : 100_000;
const steps = check ? 5000 : 100_000;
/** @type {[string, string, number[]][]} */
const workloads = [
  [`W1, the sparks: a thousand born each frame, ${count(frames)} frames`, "sparks", [frames]],
  [
    `W2, a scene graph of ${count(nodes)} nodes, a third removed and inserted again`,
    "scene",
    [nodes],
  ],
  [`W3, a budget of 256 chunks of 4 KiB, ${count(steps)} steps`, "chunks", [steps]],
  [`W4, no pool, ${count(steps)} numbers`, "plain", [steps]],
];

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; owned modules on TLSF, perasc built for release.`,
  "",
];
for (const [label, name, args] of workloads) {
  /** @type {Record<string, { js: string, bytes: { raw: number, gzip: number } }>} */
  const built = Object.fromEntries(options.map((o) => [o, build(name, o)]));
  /** @type {Record<string, { value?: string, error?: string }>} */
  const probes = Object.fromEntries(options.map((o) => [o, probe(built[o]?.js ?? "", args)]));
  const want = probes.p0?.value;
  if (want === undefined) throw new Error(`${label}, p0: ${probes.p0?.error ?? ""}`);
  const timed = options.filter((o) => probes[o]?.value === want);
  /** @type {Record<string, Loaded>} */
  const modules = {};
  for (const o of timed) modules[o] = await load(built[o]?.js ?? "");
  /** @type {Record<string, number[]>} */
  const times = Object.fromEntries(timed.map((o) => [o, []]));
  /** @type {Record<string, number>} */
  const cpu = Object.fromEntries(timed.map((o) => [o, 0]));
  /** @type {Record<string, number>} */
  const wall = Object.fromEntries(timed.map((o) => [o, 0]));
  for (let round = 0; round < ROUNDS; round++) {
    const order = round % 2 === 0 ? timed : [...timed].reverse();
    for (const o of order) {
      const c0 = process.cpuUsage();
      const t0 = performance.now();
      modules[o]?.run?.(...args);
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[o]?.push(t);
      wall[o] = (wall[o] ?? 0) + t;
      cpu[o] = (cpu[o] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  lines.push(`### ${label}`, "");
  lines.push("| Option | Median | / p0, median (range) | CPU / wall | Memory | Bytes (gzip) |");
  lines.push("| --- | ---: | --- | ---: | ---: | ---: |");
  for (const o of options) {
    const bytes = built[o]?.bytes;
    if (!timed.includes(o)) {
      const why = probes[o]?.error ?? `computes ${probes[o]?.value ?? "?"}, not ${want}`;
      lines.push(
        `| ${OPTIONS[o] ?? o} | ${why} | | | | ${String(bytes?.raw)} (${String(bytes?.gzip)}) |`,
      );
      continue;
    }
    const own = times[o] ?? [];
    const ratios = own.map((t, i) => t / (times.p0?.[i] ?? Number.NaN));
    const heap = modules[o]?.__perasHeap;
    // A module identical to p0's runs as p0's does (finding M037).
    const same =
      o !== "p0" &&
      readFileSync(built[o]?.js.replace(".measure.mjs", ".wasm") ?? "").equals(
        readFileSync(built.p0?.js.replace(".measure.mjs", ".wasm") ?? ""),
      );
    if (same) {
      lines.push(
        `| ${OPTIONS[o] ?? o} | identical to p0's module | 1.000 | | ${heap ? `${String(heap.memory() / 1024)} KiB` : "-"} | ${String(bytes?.raw)} (${String(bytes?.gzip)}) |`,
      );
      continue;
    }
    lines.push(
      `| ${OPTIONS[o] ?? o} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[o] ?? 0) / (wall[o] ?? 1)).toFixed(2)} | ${heap ? `${String(heap.memory() / 1024)} KiB` : "-"} | ${String(bytes?.raw)} (${String(bytes?.gzip)}) |`,
    );
  }
  lines.push("");
}

// Safety: the stale read.
lines.push("### Safety: a handle kept after its value was removed, then read", "");
lines.push("| Option | Read | Caught |", "| --- | ---: | --- |");
/** @type {Record<string, string>} */
const stale = {};
for (const o of options) {
  const p = probe(build("stale", o).js, []);
  const read = "value" in p ? p.value : p.error;
  stale[o] = read;
  lines.push(
    `| ${OPTIONS[o] ?? o} | ${read} | ${read === "-1" ? "yes" : "no, another value in silence"} |`,
  );
}
lines.push("");

console.log(lines.join("\n"));
if (check) {
  if (stale.p1 !== "-1" || stale.p3 !== "-1")
    throw new Error(`the stale read is not caught: p1 ${String(stale.p1)}, p3 ${String(stale.p3)}`);
} else {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# Pools: the options measured\n\nGenerated by \`node scripts/bench-pool.mjs\`, for [decision 0044](../../docs/en/decisions/0044-pools.md).\n\n${lines.join("\n")}`,
  );
}
