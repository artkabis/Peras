// Decision 0046's measure: four applications written first in TypeScript
// (bench/next/*.ts), then translated into Peras as a developer from
// TypeScript would (bench/next/*.peras, notes.apeiron). The errors met while
// translating are kept run by run in bench/next/ease.
//
// 1. Results: each Peras version, built for production (finding M029), gives
//    its TypeScript's line, twice on one instance.
// 2. Lines: of each version, blank lines and comments apart.
// 3. Time, secondary: the TypeScript, its Peras version and the TypeScript
//    again as a second module (the A/A control, finding M042), in one
//    process, the order alternating round by round; the median of the ratios
//    to the TypeScript round by round, and their range.
//
// Usage: node scripts/bench-next.mjs [--check] [--rounds N]
//        --check: small sizes, one round, the results checked, nothing written.
// Writes bench/next/RESULTS.md, unless --check.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";
import ts from "typescript";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "next");
const work = join(root, "target", "bench-next");
const perasc = join(root, "target", "release", "perasc");

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/** @param {string} s */
const lines = (s) =>
  s.split("\n").filter((l) => l.trim() !== "" && !l.trim().startsWith("//")).length;

/** @typedef {{ run(n: number): string }} Program */
/** @param {string} file @returns {Promise<Program>} */
async function load(file) {
  /** @type {unknown} */
  const loaded = await import(pathToFileURL(file).href);
  return /** @type {Program} */ (loaded);
}

/** The TypeScript as JavaScript, as a module of its own under a name. @param {string} name @param {string} as */
function transpile(name, as) {
  const source = readFileSync(join(bench, `${name}.ts`), "utf8");
  const out = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  });
  const file = join(work, `${name}-${as}.mjs`);
  writeFileSync(file, out.outputText);
  return file;
}

/** The Peras version, built for production. @param {string} name @param {string} file */
function build(name, file) {
  const out = join(work, name);
  const r = spawnSync(
    perasc,
    ["build", join(bench, file), "--out", out, "--allocator", "tlsf", "--color", "never"],
    { encoding: "utf8" },
  );
  if (r.status !== 0) throw new Error(`${name}: ${r.stderr}`);
  return join(out, `${name}.js`);
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;

/** @type {[string, string, string, number, number][]} */
const programs = [
  ["A1, notes", "notes", "notes.apeiron", 2000, 200],
  ["A2, a ledger", "ledger", "ledger.peras", 20000, 500],
  ["A3, a particle scene", "particles", "particles.peras", 300, 20],
  ["A4, a route finder", "routes", "routes.peras", 60, 10],
];

const out = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; Peras built for production.`,
  "",
  "| Program | Same result | Lines, TypeScript | Lines, Peras | Peras / TypeScript, median (range) | A/A, TypeScript / TypeScript, median (range) |",
  "| --- | --- | ---: | ---: | --- | --- |",
];
for (const [label, name, file, size, small] of programs) {
  const n = check ? small : size;
  const reference = await load(transpile(name, "a"));
  const again = await load(transpile(name, "b"));
  const peras = await load(build(name, file));
  const want = reference.run(n);
  const got = peras.run(n);
  const twice = peras.run(n);
  const same = got === want && twice === want;
  if (!same) throw new Error(`${label}: Peras gives ${got}, then ${twice}; TypeScript ${want}`);
  /** @type {Record<string, number[]>} */
  const times = { reference: [], peras: [], again: [] };
  const order = /** @type {const} */ (["reference", "peras", "again"]);
  /** @type {Record<string, Program>} */
  const modules = { reference, peras, again };
  for (let round = 0; round < ROUNDS; round++) {
    const turn = round % 2 === 0 ? order : [...order].reverse();
    for (const k of turn) {
      const t0 = performance.now();
      modules[k]?.run(n);
      times[k]?.push(performance.now() - t0);
    }
  }
  /** @param {string} k */
  const ratio = (k) => {
    const rs = (times[k] ?? []).map((t, i) => t / (times.reference?.[i] ?? Number.NaN));
    return `${median(rs).toFixed(3)} (${Math.min(...rs).toFixed(3)} to ${Math.max(...rs).toFixed(3)})`;
  };
  const tsLines = lines(readFileSync(join(bench, `${name}.ts`), "utf8"));
  const perasLines = lines(readFileSync(join(bench, file), "utf8"));
  out.push(
    `| ${label}, ${n.toLocaleString("en")} | yes, twice | ${String(tsLines)} | ${String(perasLines)} | ${ratio("peras")} | ${ratio("again")} |`,
  );
}
out.push("");
console.log(out.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# What to build next: the four applications\n\nGenerated by \`node scripts/bench-next.mjs\`, for [decision 0046](../../docs/en/decisions/0046-what-to-build-next-again.md). The errors met while translating each are in [ease](ease).\n\n${out.join("\n")}`,
  );
}
