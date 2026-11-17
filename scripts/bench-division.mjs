// Decision 0034's protocol: integer division, as a user from JavaScript
// meets it. Each option is a prototype the compiler builds when
// PERAS_PROTO_DIV names it: d0, today's; d1, the error shows the division;
// d2, `/` gives an `f64`, `a.div(b)` the integer quotient; d3, `/` on
// integers refused.
//
// 1. Ease of writing: the five programs of bench/division/ease, written as
//    a user writes them from JavaScript, fixed by applying the fix each
//    diagnostic offers, as given, until they build: the errors met, the
//    lines changed, and whether the program so fixed gives JavaScript's
//    result. Where no diagnostic offers an edit, two rules a user follows
//    are applied, registered once the first version met them (a precision
//    of the protocol, recorded in the decision): `Math` is not defined
//    (E0200), so `Math.f(x)` is written `(x).f()`, the guide's way; an
//    integer has no `floor`, `round`, `ceil` or `trunc` (E0215), being
//    whole already, so the call goes. Past that, the program is stuck, and
//    its error recorded.
// 2. The workloads: Q4 and Q1 as each option fixed them, over 10,000,000
//    pixels and steps, owned modules on TLSF in Node, in one process, the
//    order alternating round by round: the median time, its ratio to d0
//    round by round, the CPU time over the wall time, the bytes.
// Point 2 of the protocol, non-regression, is `scripts/check-regression.mjs`
// run under each option; its counts are recorded in the decision.
//
// Usage: node scripts/bench-division.mjs [--check] [--rounds N]
//        --check: one round, the results checked, nothing written.
// Writes bench/division/RESULTS.md, unless --check.
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
const bench = join(root, "bench", "division");
const work = join(root, "target", "bench-division");
const perasc = join(root, "target", "release", "perasc");
const OPTIONS = ["d0", "d1", "d2", "d3"];

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
  const dir = join(work, `${String(builds)}-${name}-${option}`);
  mkdirSync(dir, { recursive: true });
  const file = join(dir, `${name}.peras`);
  writeFileSync(file, text);
  const r = spawnSync(
    perasc,
    ["build", file, "--out", dir, "--allocator", "tlsf", "--json", "--color", "never"],
    { encoding: "utf8", env: { ...process.env, PERAS_PROTO_DIV: option } },
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
 * `Math.f(x)` written `(x).f()`, the guide's way, for the first one found.
 * @param {string} text
 */
function mathRule(text) {
  const m = /Math\.(\w+)\(/.exec(text);
  if (m === null) return undefined;
  const open = m.index + m[0].length - 1;
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    if (text[i] === "(") depth += 1;
    if (text[i] === ")") depth -= 1;
    if (depth === 0) {
      const inner = text.slice(open + 1, i);
      return `${text.slice(0, m.index)}(${inner}).${m[1] ?? ""}()${text.slice(i + 1)}`;
    }
  }
  return undefined;
}

/**
 * `.floor()` on an integer, which is whole already: the call goes.
 * @param {string} text
 * @param {Diag[]} diagnostics
 */
function wholeRule(text, diagnostics) {
  const d = diagnostics.find(
    (x) =>
      x.code === "E0215" &&
      ["floor", "round", "ceil", "trunc"].includes(x.message.args.member ?? ""),
  );
  const range = d?.labels[0]?.range;
  if (range === undefined) return undefined;
  const bytes = Buffer.from(text);
  const [start, end] = [range.start.offset, range.end.offset];
  if (
    bytes.subarray(start - 1, start).toString() !== "." ||
    bytes.subarray(end, end + 2).toString() !== "()"
  ) {
    return undefined;
  }
  return Buffer.concat([bytes.subarray(0, start - 1), bytes.subarray(end + 2)]).toString();
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
    const ruled = built.diagnostics.some((d) => d.code === "E0200")
      ? mathRule(text)
      : wholeRule(text, built.diagnostics);
    if (ruled === undefined) {
      return { text, natural, met, built: undefined, stuck: built.diagnostics[0]?.code ?? "?" };
    }
    text = ruled;
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
  ["q1", [5], "2.5"],
  ["q2", [[1, 2, 4]], String(7 / 3)],
  ["q3", [[1, 3, 5, 7, 9, 11], 9], "4"],
  ["q4", [7, 5], "315"],
  ["q5", [2, 3], "67 %"],
];
lines.push("### Ease of writing, and the results the fixes give", "");
lines.push(
  "Each program as a user writes it from JavaScript, fixed by the edits its diagnostics offer, as given, then the guide's rule for `Math`, until it builds.",
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
  const recorded = { d0: 3, d1: 1, d2: 0, d3: 3 };
  for (const o of OPTIONS) {
    if (wrong[o] !== recorded[/** @type {"d0"} */ (o)]) {
      throw new Error(
        `${o}: ${String(wrong[o])} wrong, recorded ${String(recorded[/** @type {"d0"} */ (o)])}`,
      );
    }
  }
}

// 2. The workloads: Q4 and Q1 as each option fixed them.
/** @type {[string, string, string, unknown[]][]} */
const workloads = [
  ["W1, Q4 over 10,000,000 pixels, an integer meant", "q4", "q4", [1000, 10_000]],
  ["W2, Q1 over 10,000,000 steps, a fraction meant", "q1", "q1", [10_000_000]],
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
    modules.push({
      option,
      run: () => m[fn]?.(...args),
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
  const base = modules.findIndex((m) => m.option === "d0");
  lines.push(`### ${label}`, "");
  lines.push("| Option | Result | Median | / d0, median (range) | CPU / wall | Bytes (gzip) |");
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
    `# Integer division: the options measured\n\nGenerated by \`node scripts/bench-division.mjs\`, for [decision 0034](../../docs/en/decisions/0034-integer-division.md).\n\n${lines.join("\n")}`,
  );
}
