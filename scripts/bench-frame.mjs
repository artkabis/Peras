// Decision 0047's measure: a framebuffer of pixels. Two programs, each in
// TypeScript (the specification) and in Peras for each option:
//   f0, the frame an Array<u8> as decision 0046 wrote it;
//   f1, the frame an Array<u32>, one word a pixel, as a guide would say;
//   f2, a Framebuffer written in Peras, imported (bench/frame/framebuffer.peras).
// A3 is decision 0046's particle scene (bench/next/particles.*), P2 a full
// frame (bench/frame/plasma.*).
//
// 1. Results: each version gives its TypeScript's line, twice on one
//    instance, built for production (finding M029).
// 2. Time: every version in one process, the order alternating round by
//    round; the ratio to f0 round by round, median and range, and to the
//    TypeScript. The A/A controls (finding M042): the TypeScript as a second
//    module against the first, and f0 built twice, one against the other.
// 3. Memory: each module's memory at the end of its rounds, its peak, as a
//    module's memory never shrinks; and its bytes, raw and gzip.
// 4. Lines: of each program, blank lines and comments apart; f2's library
//    counted apart, as a user does not write it.
//
// Usage: node scripts/bench-frame.mjs [--check] [--rounds N]
//        --check: small sizes, one round, the results checked, nothing written.
// Writes bench/frame/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";
import ts from "typescript";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const work = join(root, "target", "bench-frame");
const perasc = join(root, "target", "release", "perasc");

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/** @param {string} s */
const lines = (s) =>
  s.split("\n").filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;

/** @typedef {{ run(n: number): string, __perasHeap?: { memory(): number } }} Program */
/** @param {string} file @returns {Promise<Program>} */
async function load(file) {
  /** @type {unknown} */
  const loaded = await import(pathToFileURL(file).href);
  return /** @type {Program} */ (loaded);
}

/** The TypeScript as JavaScript, a module of its own under a name. @param {string} source @param {string} as */
function transpile(source, as) {
  const out = ts.transpileModule(readFileSync(join(root, source), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  });
  const file = join(work, `${as}.mjs`);
  writeFileSync(file, out.outputText);
  return { js: file, bytes: null };
}

/** A Peras version, built for production, with its memory readable. @param {string} source @param {string} as */
function build(source, as) {
  const out = join(work, as);
  const r = spawnSync(
    perasc,
    ["build", join(root, source), "--out", out, "--allocator", "tlsf", "--color", "never"],
    { encoding: "utf8" },
  );
  if (r.status !== 0) throw new Error(`${as}: ${r.stderr}`);
  const name =
    source
      .split("/")
      .pop()
      ?.replace(/\.peras$/, "") ?? "";
  const text = readFileSync(join(out, `${name}.js`), "utf8").replace(
    "export const __perasHeap = {",
    "export const __perasHeap = {\n  memory: () => $runtime.memory.buffer.byteLength,",
  );
  const js = join(out, `${name}.measure.mjs`);
  writeFileSync(js, text);
  const wasm = readFileSync(join(out, `${name}.wasm`));
  return { js, bytes: { raw: wasm.length, gzip: gzipSync(wasm).length } };
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
/** @param {number[]} rs */
const spread = (rs) =>
  `${median(rs).toFixed(3)} (${Math.min(...rs).toFixed(3)} to ${Math.max(...rs).toFixed(3)})`;

const programs = [
  {
    label: "A3, the particle scene",
    size: 300,
    small: 20,
    ts: "bench/next/particles.ts",
    peras: {
      f0: "bench/next/particles.peras",
      f1: "bench/frame/particles-f1.peras",
      f2: "bench/frame/particles-f2.peras",
    },
  },
  {
    label: "P2, a full frame",
    size: 120,
    small: 4,
    ts: "bench/frame/plasma.ts",
    peras: {
      f0: "bench/frame/plasma-f0.peras",
      f1: "bench/frame/plasma-f1.peras",
      f2: "bench/frame/plasma-f2.peras",
    },
  },
];

const out = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; Peras built for production.`,
  "",
];
for (const p of programs) {
  const n = check ? p.small : p.size;
  /** @type {Record<string, { js: string, bytes: { raw: number, gzip: number } | null }>} */
  const built = {
    ts: transpile(p.ts, `${p.ts.split("/").pop() ?? ""}-a`),
    "ts again": transpile(p.ts, `${p.ts.split("/").pop() ?? ""}-b`),
    f0: build(p.peras.f0, `${p.label.slice(0, 2)}-f0`),
    "f0 again": build(p.peras.f0, `${p.label.slice(0, 2)}-f0b`),
    f1: build(p.peras.f1, `${p.label.slice(0, 2)}-f1`),
    f2: build(p.peras.f2, `${p.label.slice(0, 2)}-f2`),
  };
  const names = Object.keys(built);
  /** @type {Record<string, Program>} */
  const modules = {};
  for (const k of names) modules[k] = await load(built[k]?.js ?? "");
  const want = modules.ts?.run(n);
  for (const k of names) {
    const a = modules[k]?.run(n);
    const b = modules[k]?.run(n);
    if (a !== want || b !== want)
      throw new Error(
        `${p.label}, ${k}: ${String(a)}, then ${String(b)}; TypeScript ${String(want)}`,
      );
  }
  /** @type {Record<string, number[]>} */
  const times = Object.fromEntries(names.map((k) => [k, []]));
  for (let round = 0; round < ROUNDS; round++) {
    const turn = round % 2 === 0 ? names : [...names].reverse();
    for (const k of turn) {
      const t0 = performance.now();
      modules[k]?.run(n);
      times[k]?.push(performance.now() - t0);
    }
  }
  /** @param {string} k @param {string} by */
  const ratios = (k, by) => (times[k] ?? []).map((t, i) => t / (times[by]?.[i] ?? Number.NaN));
  out.push(`### ${p.label}, ${n.toLocaleString("en")}`, "");
  out.push(
    "| Version | Same result | Lines | Time / f0, median (range) | Time / TypeScript, median | Memory at the end | Bytes, gzip |",
    "| --- | --- | ---: | --- | ---: | ---: | ---: |",
  );
  for (const k of names) {
    const source = k.startsWith("ts")
      ? p.ts
      : p.peras[/** @type {"f0"|"f1"|"f2"} */ (k.slice(0, 2))];
    const memory = modules[k]?.__perasHeap?.memory();
    const bytes = built[k]?.bytes;
    out.push(
      `| ${k} | yes, twice | ${String(lines(readFileSync(join(root, source), "utf8")))} | ${spread(ratios(k, "f0"))} | ${median(ratios(k, "ts")).toFixed(3)} | ${memory === undefined ? "–" : `${String(memory / 1024)} KiB`} | ${bytes ? String(bytes.gzip) : "–"} |`,
    );
  }
  out.push("");
}
out.push(
  `f2's library, \`bench/frame/framebuffer.peras\`: ${String(lines(readFileSync(join(root, "bench/frame/framebuffer.peras"), "utf8")))} lines, written once.`,
  "",
);
console.log(out.join("\n"));
if (!check) {
  writeFileSync(
    join(root, "bench", "frame", "RESULTS.md"),
    `# A framebuffer of pixels\n\nGenerated by \`node scripts/bench-frame.mjs\`, for [decision 0047](../../docs/en/decisions/0047-framebuffer.md). The errors met while writing each version are in [ease](ease).\n\n${out.join("\n")}`,
  );
}
