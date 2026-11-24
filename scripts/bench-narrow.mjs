// Decision 0045's protocol: narrowing an option through `&&`, `||` and `!`.
// Each option is a prototype the compiler builds when PERAS_PROTO_NARROW is
// `n1` (chains) or `n2` (facts); without it, n0, as today. The programs of
// bench/narrow are written as in TypeScript, those of bench/narrow/n0 as n0
// asks.
//
// 1. Ease: the errors `perasc check --json` reports for each TypeScript form
//    under each option, and the lines of the form that builds.
// 2. Each program built for production: under n0 its n0 form, under n1 and
//    n2 its TypeScript form where it builds; each run first in a child
//    process with a time limit, twice on one instance, its result compared
//    with n0's.
// 3. The modules that compute n0's result, in one process, the order
//    alternating round by round (finding M017): the median time, its ratio
//    to n0 round by round, the CPU time over the wall time, the module's
//    memory at the end, its bytes, raw and gzip; a module identical to n0's
//    counted at 1.000 (finding M037).
//
// Usage: node scripts/bench-narrow.mjs [--check] [--rounds N]
//        --check: small sizes, one round, the results checked, nothing written.
// Writes bench/narrow/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "narrow");
const work = join(root, "target", "bench-narrow");
const perasc = join(root, "target", "release", "perasc");
/** @type {Record<string, string>} */
const OPTIONS = {
  n0: "n0, as today",
  n1: "n1, chains",
  n2: "n2, facts",
};
const options = Object.keys(OPTIONS);

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/** @param {string} option */
const envOf = (option) => ({ ...process.env, PERAS_PROTO_NARROW: option === "n0" ? "" : option });

/** The errors the compiler reports for a file under an option. @param {string} file @param {string} option */
function errors(file, option) {
  const r = spawnSync(perasc, ["check", file, "--json", "--lang", "en"], {
    encoding: "utf8",
    env: envOf(option),
  });
  /** @type {unknown} */
  const parsed = JSON.parse(r.stdout);
  const report = /** @type {{ errorCount?: number, error_count?: number }} */ (parsed);
  return report.errorCount ?? report.error_count ?? Number.NaN;
}

/** @param {string} s */
const lines = (s) =>
  s.split("\n").filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;

/**
 * Builds a file with an option, for production.
 * @param {string} file @param {string} name @param {string} option
 */
function build(file, name, option) {
  const dir = join(work, `${name}-${option}`);
  const args = ["build", file, "--out", dir, "--color", "never", "--allocator", "tlsf"];
  const r = spawnSync(perasc, args, { encoding: "utf8", env: envOf(option) });
  if (r.status !== 0) throw new Error(`${name}, ${option}: ${r.stderr}`);
  const text = readFileSync(join(dir, `${name}.js`), "utf8").replace(
    "export const __perasHeap = {",
    "export const __perasHeap = {\n  memory: () => $runtime.memory.buffer.byteLength,",
  );
  const measured = join(dir, `${name}.measure.mjs`);
  writeFileSync(measured, text);
  const wasm = readFileSync(join(dir, `${name}.wasm`));
  return { js: measured, wasm, bytes: { raw: wasm.length, gzip: gzipSync(wasm).length } };
}

/** The result of a module called twice, in a child process, or why none came. @param {string} js @param {number} n */
function probe(js, n) {
  const code = `import(${JSON.stringify(pathToFileURL(js).href)}).then((m) => {
    const a = m.run(${String(n)}); const b = m.run(${String(n)});
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

/** @typedef {Record<string, (...args: unknown[]) => unknown> & { __perasHeap?: { memory(): number } }} Loaded */
/** @param {string} js @returns {Promise<Loaded>} */
async function load(js) {
  /** @type {unknown} */
  const loaded = await import(pathToFileURL(js).href);
  return /** @type {Loaded} */ (loaded);
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
const size = check ? 2000 : 200_000;
/** @type {[string, string, number][]} */
const programs = [
  ["N1, the parents", "links", size],
  ["N2, the pull", "pull", size],
  ["N3, both there", "both", size * 10],
  ["N4, the tally", "tally", size],
  ["N5, a field", "weigh", size],
];

const lines0 = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; owned modules on TLSF, perasc built for release.`,
  "",
  "### Ease: the errors met by the TypeScript form, and the lines of the form that builds",
  "",
  "| Program | n0 | n1 | n2 |",
  "| --- | --- | --- | --- |",
];
/** @type {Record<string, Record<string, number>>} */
const met = {};
/** @type {Record<string, number>} */
const total = { n0: 0, n1: 0, n2: 0 };
for (const [label, name] of programs) {
  const ts = join(bench, `${name}.peras`);
  const n0 = join(bench, "n0", `${name}.peras`);
  const cells = options.map((o) => {
    const e = errors(ts, o);
    met[name] = { ...met[name], [o]: e };
    total[o] = (total[o] ?? 0) + e;
    const file = e === 0 ? ts : n0;
    return `${String(e)} errors, ${String(lines(readFileSync(file, "utf8")))} lines`;
  });
  lines0.push(`| ${label} | ${cells.join(" | ")} |`);
}
lines0.push(`| All | ${options.map((o) => `${String(total[o])} errors`).join(" | ")} |`, "");

for (const [label, name, n] of programs) {
  const ts = join(bench, `${name}.peras`);
  /** @type {Record<string, ReturnType<typeof build>>} */
  const built = {};
  built.n0 = build(join(bench, "n0", `${name}.peras`), name, "n0");
  for (const o of ["n1", "n2"]) if (met[name]?.[o] === 0) built[o] = build(ts, name, o);
  const present = options.filter((o) => built[o] !== undefined);
  /** @type {Record<string, { value?: string, error?: string }>} */
  const probes = Object.fromEntries(present.map((o) => [o, probe(built[o]?.js ?? "", n)]));
  const want = probes.n0?.value;
  if (want === undefined) throw new Error(`${label}, n0: ${probes.n0?.error ?? ""}`);
  const timed = present.filter((o) => probes[o]?.value === want);
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
      modules[o]?.run?.(n);
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[o]?.push(t);
      wall[o] = (wall[o] ?? 0) + t;
      cpu[o] = (cpu[o] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  lines0.push(`### ${label}, ${n.toLocaleString("en")}`, "");
  lines0.push("| Option | Median | / n0, median (range) | CPU / wall | Memory | Bytes (gzip) |");
  lines0.push("| --- | ---: | --- | ---: | ---: | ---: |");
  for (const o of options) {
    const b = built[o];
    if (b === undefined) {
      lines0.push(`| ${OPTIONS[o] ?? o} | refused, ${String(met[name]?.[o])} errors | | | | |`);
      continue;
    }
    const bytes = `${String(b.bytes.raw)} (${String(b.bytes.gzip)})`;
    if (!timed.includes(o)) {
      const why = probes[o]?.error ?? `computes ${probes[o]?.value ?? "?"}, not ${want}`;
      lines0.push(`| ${OPTIONS[o] ?? o} | ${why} | | | | ${bytes} |`);
      continue;
    }
    const heap = modules[o]?.__perasHeap;
    const memory = heap ? `${String(heap.memory() / 1024)} KiB` : "-";
    if (o !== "n0" && b.wasm.equals(built.n0.wasm)) {
      lines0.push(
        `| ${OPTIONS[o] ?? o} | identical to n0's module | 1.000 | | ${memory} | ${bytes} |`,
      );
      continue;
    }
    const own = times[o] ?? [];
    const ratios = own.map((t, i) => t / (times.n0?.[i] ?? Number.NaN));
    lines0.push(
      `| ${OPTIONS[o] ?? o} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[o] ?? 0) / (wall[o] ?? 1)).toFixed(2)} | ${memory} | ${bytes} |`,
    );
  }
  lines0.push("");
  if (check) {
    for (const o of present)
      if (probes[o]?.value !== want)
        throw new Error(
          `${label}, ${o}: ${probes[o]?.error ?? probes[o]?.value ?? ""}, not ${want}`,
        );
  }
}

console.log(lines0.join("\n"));
if (check) {
  // n2 takes every program of the protocol; n1 fewer errors than n0.
  if (total.n2 !== 0 || (total.n1 ?? 0) >= (total.n0 ?? 0))
    throw new Error(`the errors met: ${JSON.stringify(met)}`);
} else {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# Narrowing: the options measured\n\nGenerated by \`node scripts/bench-narrow.mjs\`, for [decision 0045](../../docs/en/decisions/0045-narrowing.md).\n\n${lines0.join("\n")}`,
  );
}
