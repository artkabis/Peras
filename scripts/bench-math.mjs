// Decision 0035's protocol: `Math` as JavaScript writes it, and rounding
// an integer. a2 and b3, chosen, are the build's; a0 and b0, the errors as
// they were, are built when PERAS_PROTO_MATH and PERAS_PROTO_WHOLE name
// them. a1, b1 and b2 were measured at commit 6cb3444 and removed once
// not chosen: their figures are in the decision.
//
// 1. Ease of writing: the six programs of bench/math/ease, written as a
//    user writes them from JavaScript, fixed by applying the edits their
//    diagnostics offer, as given, nothing else, until they build; a program
//    whose diagnostics offer none is stuck. For each pair: the errors met,
//    the lines changed, and whether the program gives JavaScript's result.
// 2. The workloads: R4 over 10,000,000 pixels, and R6 over 10,000,000
//    calls (R1 returns a string: its calls would measure the boundary, a
//    precision recorded before the measure), as each pair fixed them, owned
//    modules on TLSF in Node, in one process, the order alternating round
//    by round: the median time, its ratio to the first pair that builds
//    them, the CPU time over the wall time, the bytes.
// Point 2 of the protocol, non-regression, is `scripts/check-regression.mjs`
// run under each option.
//
// Usage: node scripts/bench-math.mjs [--check] [--rounds N]
//        --check: one round, the results checked, nothing written.
// Writes bench/math/RESULTS.md, unless --check.
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "math");
const work = join(root, "target", "bench-math");
const perasc = join(root, "target", "release", "perasc");
/** The pairs measured: an option of each question. */
const OPTIONS = ["a0 b0", "a2 b3"];
/** @param {string} pair */
const envOf = (pair) => {
  const [math, whole] = pair.split(" ");
  return { ...process.env, PERAS_PROTO_MATH: math ?? "a0", PERAS_PROTO_WHOLE: whole ?? "b0" };
};

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/**
 * @typedef {{ code: string, message: { args: Record<string, string> }, labels: { range: { start: { offset: number }, end: { offset: number } } }[], fix?: { edits: { range: { start: { offset: number }, end: { offset: number } }, replacement: string }[] } }} Diag
 */

let builds = 0;
/**
 * Builds a source text under an option.
 * @param {string} name
 * @param {string} text
 * @param {string} option
 * @returns {{ ok: true, js: string, wasm: Buffer } | { ok: false, diagnostics: Diag[] }}
 */
function build(name, text, option) {
  builds += 1;
  const dir = join(work, `${String(builds)}-${name}-${option.replace(" ", "-")}`);
  mkdirSync(dir, { recursive: true });
  const file = join(dir, `${name}.peras`);
  writeFileSync(file, text);
  const r = spawnSync(
    perasc,
    ["build", file, "--out", dir, "--allocator", "tlsf", "--json", "--color", "never"],
    { encoding: "utf8", env: envOf(option) },
  );
  if (r.status === 0) {
    return { ok: true, js: join(dir, `${name}.js`), wasm: readFileSync(join(dir, `${name}.wasm`)) };
  }
  /** @type {unknown} */
  const parsed = JSON.parse(r.stdout);
  const json = /** @type {{ files: { diagnostics: Diag[] }[] }} */ (parsed);
  return { ok: false, diagnostics: json.files.flatMap((f) => f.diagnostics) };
}

/**
 * A program fixed under an option: by the edits its diagnostics offer, then
 * the guide's rule for `Math`.
 * @param {string} name
 * @param {string} option
 */
function fix(name, option) {
  const natural = readFileSync(join(bench, "ease", `${name}.peras`), "utf8");
  let text = natural;
  /** @type {string[]} */
  const met = [];
  for (let step = 0; step < 12; step++) {
    const built = build(name, text, option);
    if (built.ok) return { text, natural, met, built, stuck: undefined };
    met.push(...built.diagnostics.map((d) => d.code));
    const offered = built.diagnostics.find((d) => (d.fix?.edits.length ?? 0) > 0);
    if (offered?.fix !== undefined) {
      const bytes = Buffer.from(text);
      const edits = [...offered.fix.edits].sort(
        (a, b) => b.range.start.offset - a.range.start.offset,
      );
      let out = bytes;
      for (const e of edits) {
        out = Buffer.concat([
          out.subarray(0, e.range.start.offset),
          Buffer.from(e.replacement),
          out.subarray(e.range.end.offset),
        ]);
      }
      text = out.toString();
      continue;
    }
    return { text, natural, met, built: undefined, stuck: built.diagnostics[0]?.code ?? "?" };
  }
  return { text, natural, met, built: undefined, stuck: "loop" };
}

/**
 * The lines two texts differ by: removed plus added.
 * @param {string} a
 * @param {string} b
 */
function changed(a, b) {
  const x = a.split("\n");
  const y = b.split("\n");
  return x.filter((l) => !y.includes(l)).length + y.filter((l) => !x.includes(l)).length;
}

/** @param {string} js */
async function load(js) {
  /** @type {unknown} */
  const m = await import(js);
  return /** @type {Record<string, (...a: unknown[]) => unknown>} */ (m);
}

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds.`,
  "",
];

// 1. Ease of writing, and the results the fixes give. JavaScript's result
// for each call, as the program written from JavaScript means it.
/** @type {[string, unknown[], string][]} */
const programs = [
  ["r1", [0, 0, 3, -4], "5.00 -53.1"],
  ["r2", [300], "255"],
  ["r3", [[1, 3, 5, 7, 9, 11], 9], "4"],
  ["r4", [7, 5], "315"],
  ["r5", [2, 3], "67 %"],
  ["r6", [0.125], "0.13"],
];
lines.push("### Ease of writing, and the results the fixes give", "");
lines.push(
  "Each program as a user writes it from JavaScript, fixed by the edits its diagnostics offer, as given, nothing else, until it builds.",
  "",
);
lines.push("| Program | Option | Errors met | Lines changed | Result (JavaScript's) | Right |");
lines.push("| --- | --- | --- | ---: | --- | --- |");
/** @type {Record<string, Record<string, string>>} */
const fixed = {};
/** @type {Record<string, number>} */
const wrong = Object.fromEntries(OPTIONS.map((o) => [o, 0]));
for (const [name, args, want] of programs) {
  for (const option of OPTIONS) {
    const f = fix(name, option);
    let got = "-";
    if (f.built !== undefined) {
      const m = await load(f.built.js);
      got = String(m[name]?.(...globalThis.structuredClone(args)));
      (fixed[name] ??= {})[option] = f.text;
    }
    const right = got === want;
    if (!right) wrong[option] = (wrong[option] ?? 0) + 1;
    const met = f.met.length === 0 ? "builds" : f.met.join(", ");
    const result = f.stuck === undefined ? got : `stuck at ${f.stuck}`;
    lines.push(
      `| ${name} | ${option} | ${met} | ${String(changed(f.natural, f.text))} | ${result} (${want}) | ${right ? "yes" : "no"} |`,
    );
  }
}
lines.push("");
lines.push(
  `Programs not giving JavaScript's result once fixed: ${OPTIONS.map((o) => `${o}, ${String(wrong[o])}`).join("; ")}.`,
  "",
);
if (check) {
  // The counts the prototypes gave when committed, checked from then on.
  // The counts the prototypes gave when committed, checked from then on.
  /** @type {Record<string, number>} */
  const recorded = { "a0 b0": 6, "a2 b3": 0 };
  for (const o of OPTIONS) {
    if (recorded[o] !== undefined && wrong[o] !== recorded[o]) {
      throw new Error(`${o}: ${String(wrong[o])} wrong, recorded ${String(recorded[o])}`);
    }
  }
}

// 2. The workloads: Q4 and Q1 as each option fixed them.
/** @type {[string, string, string, unknown[]][]} */
const workloads = [
  ["W1, R4 over 10,000,000 pixels", "r4", "r4", [1000, 10_000]],
  ["W2, R6 over 10,000,000 calls", "r6", "r6", [0.125]],
];
for (const [label, name, fn, args] of workloads) {
  /** @type {{ option: string, run: () => unknown, bytes: { raw: number, gzip: number } }[]} */
  const modules = [];
  for (const option of OPTIONS) {
    const text = fixed[name]?.[option];
    if (text === undefined) continue;
    const built = build(name, text, option);
    if (!built.ok) throw new Error(`${label}, ${option}: does not build`);
    const m = await load(built.js);
    const calls = name === "r6" ? 10_000_000 : 1;
    modules.push({
      option,
      run: () => {
        let last;
        for (let k = 0; k < calls; k++) last = m[fn]?.(...args);
        return last;
      },
      bytes: { raw: built.wasm.length, gzip: gzipSync(built.wasm).length },
    });
  }
  const times = modules.map(() => /** @type {number[]} */ ([]));
  const cpu = modules.map(() => 0);
  const wall = modules.map(() => 0);
  for (let round = 0; round < ROUNDS; round++) {
    const order = modules.map((_, i) => i);
    if (round % 2 === 1) order.reverse();
    for (const i of order) {
      const c0 = process.cpuUsage();
      const t0 = performance.now();
      modules[i]?.run();
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[i]?.push(t);
      wall[i] = (wall[i] ?? 0) + t;
      cpu[i] = (cpu[i] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  const median = (/** @type {number[]} */ xs) =>
    [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
  const base = 0;
  lines.push(`### ${label}`, "");
  lines.push("| Pair | Result | Median | / first, median (range) | CPU / wall | Bytes (gzip) |");
  lines.push("| --- | --- | ---: | --- | ---: | ---: |");
  modules.forEach(({ option, run, bytes }, i) => {
    const own = times[i] ?? [];
    const ratios = own.map((t, k) => t / (times[base]?.[k] ?? Number.NaN));
    lines.push(
      `| ${option} | ${String(run())} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[i] ?? 0) / (wall[i] ?? 1)).toFixed(2)} | ${String(bytes.raw)} (${String(bytes.gzip)}) |`,
    );
  });
  lines.push("");
}

console.log(lines.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# \`Math\` as JavaScript writes it: the options measured\n\nGenerated by \`node scripts/bench-math.mjs\`, for [decision 0035](../../docs/en/decisions/0035-math-as-javascript-writes-it.md).\n\n${lines.join("\n")}`,
  );
}
