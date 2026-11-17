// Finding F221's profile by removing: why the pond's managed module takes
// 2.41 times TypeScript's time (decision 0029). Each variant removes one
// more difference from the module the study measured, the rest unchanged:
//
//   M0  the managed module as the study built it: no build option, so every
//       integer operation checked for overflow;
//   M1  built for production (`--allocator tlsf`): no overflow check, the
//       indices still checked;
//   M2  M1, each index read and write (`$at`, `$put`) written as a direct
//       access;
//   M3  M2, each `$saturate(Math.floor(e), …)` written `Math.floor(e)`, as
//       pond.ts writes it;
//   M4  M3, the six arrays of the projected points made as pond.ts makes
//       them, `new Float64Array(vertices)`, not filled by `push`.
//
// TypeScript's module and the five variants in one process, frame 600 at
// 1024 × 768 with 1,000 lanterns, the order alternating round by round; the
// ratio of each to TypeScript round by round, its median and range. Every
// variant must give TypeScript's pixels.
//
// Usage: node scripts/profile-managed.mjs [--check] [--rounds N]
//        --check: one round at 256 × 192, the pixels checked.
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import ts from "typescript";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 15);
const [WIDTH, HEIGHT] = check ? [256, 192] : [1024, 768];
const root = join(import.meta.dirname, "..");
const dir = join(root, "bench", "lanterns");
const work = join(root, "target", "profile-managed");
const perasc = join(root, "target", "release", "perasc");

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/** @param {string[]} args */
function build(args) {
  const r = spawnSync(perasc, args, { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`perasc ${args.join(" ")}\n${r.stderr}`);
}

writeFileSync(
  join(work, "ts.js"),
  ts.transpileModule(readFileSync(join(dir, "pond.ts"), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  }).outputText,
);
writeFileSync(join(work, "pond.apeiron"), readFileSync(join(dir, "pond.peras")));
build(["build", join(work, "pond.apeiron"), "--out", join(work, "m0"), "--color", "never"]);
build([
  "build",
  join(work, "pond.apeiron"),
  "--out",
  join(work, "m1"),
  "--allocator",
  "tlsf",
  "--color",
  "never",
]);

/**
 * Replaces every match of a pattern, and stops the run if there is none.
 * @param {string} text
 * @param {RegExp} pattern
 * @param {string | ((...m: string[]) => string)} by
 * @param {string} what
 */
function replaced(text, pattern, by, what) {
  const count = [...text.matchAll(pattern)].length;
  if (count === 0) throw new Error(`${what}: nothing to replace`);
  // @ts-expect-error: a string or a function, as String.replace takes.
  return { text: text.replace(pattern, by), count };
}

/**
 * Finds the end of the call whose arguments open at `open`.
 * @param {string} s
 * @param {number} open
 */
function closing(s, open) {
  let depth = 0;
  for (let i = open; i < s.length; i++) {
    if (s[i] === "(") depth++;
    else if (s[i] === ")" && --depth === 0) return i;
  }
  throw new Error("unbalanced");
}

/**
 * Splits the arguments of a call at its top-level commas.
 * @param {string} s
 */
function args(s) {
  const out = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < s.length; i++) {
    if (s[i] === "(" || s[i] === "[") depth++;
    else if (s[i] === ")" || s[i] === "]") depth--;
    else if (s[i] === "," && depth === 0) {
      out.push(s.slice(start, i).trim());
      start = i + 1;
    }
  }
  out.push(s.slice(start).trim());
  return out;
}

/**
 * Rewrites every call of `name` by `by(arguments)`, innermost last.
 * @param {string} text
 * @param {string} name
 * @param {(a: string[]) => string} by
 */
function rewriteCalls(text, name, by) {
  let count = 0;
  let at = text.lastIndexOf(`${name}(`);
  while (at >= 0) {
    // A definition (`const $at = (`) is not a call.
    if (!text.slice(Math.max(0, at - 6), at).includes("const ")) {
      const open = at + name.length;
      const end = closing(text, open);
      text = `${text.slice(0, at)}${by(args(text.slice(open + 1, end)))}${text.slice(end + 1)}`;
      count++;
    }
    at = text.lastIndexOf(`${name}(`, at - 1);
  }
  if (count === 0) throw new Error(`${name}: nothing to replace`);
  return { text, count };
}

const counts = [];
let m = readFileSync(join(work, "m1", "pond.js"), "utf8");
const at = rewriteCalls(m, "$at", ([xs, i]) => `${String(xs)}[${String(i)}]`);
const put = rewriteCalls(
  at.text,
  "$put",
  ([xs, i, v]) => `(${String(xs)}[${String(i)}] = ${String(v)})`,
);
m = put.text;
counts.push(`M2: ${String(at.count)} reads and ${String(put.count)} writes`);
writeFileSync(join(work, "m2.js"), m);
const sat = rewriteCalls(m, "$saturate", ([x]) => String(x));
m = sat.text;
counts.push(`M3: ${String(sat.count)} conversions`);
writeFileSync(join(work, "m3.js"), m);
let arrays = 0;
for (const name of ["sx", "sy", "sz", "wx", "wy", "wz"]) {
  const made = replaced(
    m,
    new RegExp(`let ${name} = \\[\\];`, "g"),
    `let ${name} = new Float64Array(vertices);`,
    name,
  );
  const pushed = replaced(made.text, new RegExp(`\\s*${name}\\.push\\(0\\.0\\);`, "g"), "", name);
  m = pushed.text;
  arrays += made.count;
}
counts.push(`M4: ${String(arrays)} arrays`);
writeFileSync(join(work, "m4.js"), m);

/** @typedef {{ render: (...a: unknown[]) => number }} Pond */
/** @type {[string, string][]} */
const files = [
  ["TypeScript", join(work, "ts.js")],
  ["M0", join(work, "m0", "pond.js")],
  ["M1", join(work, "m1", "pond.js")],
  ["M2", join(work, "m2.js")],
  ["M3", join(work, "m3.js")],
  ["M4", join(work, "m4.js")],
];
const versions = [];
for (const [label, file] of files) {
  /** @type {unknown} */
  const loaded = await import(file);
  const pond = /** @type {Pond} */ (loaded);
  const pixels = new Int32Array(WIDTH * HEIGHT);
  const depth = new Float64Array(WIDTH * HEIGHT);
  const light = new Float64Array(3 * WIDTH * HEIGHT);
  const draw = () => pond.render(pixels, depth, light, WIDTH, HEIGHT, 1000, 600);
  const filled = draw();
  const digest = `${String(filled)} ${createHash("sha256").update(Buffer.from(pixels.buffer)).digest("hex")}`;
  versions.push({ label, draw, digest });
}
for (const v of versions) {
  if (v.digest !== versions[0]?.digest)
    throw new Error(`${v.label}: ${v.digest}, not TypeScript's ${String(versions[0]?.digest)}`);
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
const times = versions.map(() => /** @type {number[]} */ ([]));
for (let round = 0; round < ROUNDS; round++) {
  const order = versions.map((_, i) => i);
  if (round % 2 === 1) order.reverse();
  for (const i of order) {
    const t0 = performance.now();
    versions[i]?.draw();
    times[i]?.push(performance.now() - t0);
  }
}
const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}; Node ${process.version}; ${String(ROUNDS)} rounds; frame 600 at ${String(WIDTH)} × ${String(HEIGHT)}. Every variant gives TypeScript's pixels. Replaced: ${counts.join("; ")}.`,
  "",
  "| Version | Median | / TypeScript, median (range) |",
  "| --- | ---: | --- |",
];
versions.forEach(({ label }, i) => {
  const own = times[i] ?? [];
  const ratios = own.map((t, k) => t / (times[0]?.[k] ?? Number.NaN));
  lines.push(
    `| ${label} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) |`,
  );
});
console.log(lines.join("\n"));
