// The owner's rule of 2026-11-17: every decision is an improvement, never a
// regression. Run before a decision lands, against the commit before it:
//
// 1. Every module of the benchmarks and examples, owned on both allocators
//    and managed, built by both compilers: identical bytes, or fewer. A
//    module identical runs at the same speed, with the same load on the
//    processor and the same memory; a module that changes must be measured
//    by the decision itself, and is listed. A module larger fails.
// 2. The compiler's time on all of them, both compilers alternating round
//    by round (finding M017): the median of the ratios. Above 1.05 fails.
// 3. The size of the compiler, both: reported.
//
// Usage: node scripts/check-regression.mjs <ref>
// The compiler of <ref> is built once, in its own target directory.
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { join } from "node:path";
import { performance } from "node:perf_hooks";

const ref = process.argv[2];
if (ref === undefined) {
  console.error("usage: node scripts/check-regression.mjs <ref>");
  process.exit(2);
}
const root = join(import.meta.dirname, "..");
const run = (/** @type {string} */ cmd, /** @type {string[]} */ args, cwd = root) =>
  spawnSync(cmd, args, { cwd, encoding: "utf8" });
const sha = run("git", ["rev-parse", ref]).stdout.trim();
if (sha === "") {
  console.error(`unknown ref ${ref}`);
  process.exit(2);
}
const target = join(root, "target", "regression", sha.slice(0, 12));
const before = join(target, "release", "perasc");
const now = join(root, "target", "release", "perasc");
if (!existsSync(before)) {
  const src = join(root, "target", "regression", "src");
  rmSync(src, { recursive: true, force: true });
  run("git", ["worktree", "prune"]);
  if (run("git", ["worktree", "add", "-f", src, sha]).status !== 0) throw new Error("worktree");
  const built = run(
    "cargo",
    ["build", "-q", "--release", "-p", "perasc", "--target-dir", target],
    src,
  );
  run("git", ["worktree", "remove", "--force", src]);
  if (built.status !== 0) throw new Error(`the compiler of ${ref} does not build`);
}
if (run("cargo", ["build", "-q", "--release", "-p", "perasc"]).status !== 0) {
  throw new Error("the compiler does not build");
}

const sources = run("git", [
  "ls-files",
  "bench/**/*.peras",
  "examples/**/*.peras",
  "bench/**/*.apeiron",
  "examples/**/*.apeiron",
])
  .stdout.trim()
  .split("\n");
const work = join(root, "target", "regression", "out");
// Every variable a prototype reads is unset: each compiler builds as it
// ships.
const env = Object.fromEntries(
  Object.entries(process.env).filter(([k]) => !k.startsWith("PERAS_PROTO_")),
);

/**
 * The bytes of every file a build writes, by name.
 * @param {string} compiler
 * @param {string} source
 * @param {string[]} extra
 * @param {string} out
 */
function built(compiler, source, extra, out) {
  rmSync(out, { recursive: true, force: true });
  const r = spawnSync(compiler, ["build", source, "--out", out, "--color", "never", ...extra], {
    cwd: root,
    env,
  });
  if (r.status !== 0) return undefined;
  return Object.fromEntries(readdirSync(out).map((f) => [f, readFileSync(join(out, f))]));
}

// 1. The modules.
let compared = 0;
let refused = 0;
const larger = [];
const smaller = [];
const changed = [];
const status = [];
for (const source of sources) {
  const kinds = source.endsWith(".apeiron")
    ? [[]]
    : [
        ["--allocator", "tlsf"],
        ["--allocator", "debug"],
      ];
  for (const extra of kinds) {
    const a = built(before, source, extra, join(work, "a"));
    const b = built(now, source, extra, join(work, "b"));
    const label = `${source} ${extra.join(" ")}`.trim();
    if ((a === undefined) !== (b === undefined)) {
      status.push(`${label}: ${a === undefined ? "builds now only" : "no longer builds"}`);
      continue;
    }
    if (a === undefined || b === undefined) {
      refused += 1;
      continue;
    }
    compared += 1;
    const size = (/** @type {Record<string, Buffer>} */ files) =>
      Object.values(files).reduce((n, f) => n + f.length, 0);
    const same =
      Object.keys(a).length === Object.keys(b).length &&
      Object.entries(a).every(([f, bytes]) => b[f]?.equals(bytes));
    if (same) continue;
    const [x, y] = [size(a), size(b)];
    const line = `${label}: ${String(x)} → ${String(y)} bytes`;
    if (y > x) larger.push(line);
    else if (y < x) smaller.push(line);
    else changed.push(line);
  }
}

// 2. The compiler's time, both alternating.
const rounds = 11;
/** @type {Record<string, number[]>} */
const times = { before: [], now: [] };
for (let round = 0; round < rounds; round++) {
  const order = round % 2 === 0 ? ["before", "now"] : ["now", "before"];
  for (const k of order) {
    const compiler = k === "before" ? before : now;
    const t0 = performance.now();
    for (const source of sources) {
      spawnSync(compiler, ["build", source, "--out", join(work, "t"), "--color", "never"], {
        cwd: root,
        env,
      });
    }
    times[k]?.push(performance.now() - t0);
  }
}
const ratios = (times.now ?? []).map((t, i) => t / (times.before?.[i] ?? Number.NaN));
const median = [...ratios].sort((a, b) => a - b)[Math.floor(ratios.length / 2)] ?? Number.NaN;

// 3. The compiler's size.
const [sizeBefore, sizeNow] = [statSync(before).size, statSync(now).size];

const out = [
  `Against ${ref} (${sha.slice(0, 7)}):`,
  `- modules: ${String(compared)} compared, ${String(larger.length)} larger, ${String(smaller.length)} smaller, ${String(changed.length)} changed at the same size, ${String(status.length)} that build in one compiler only, ${String(refused)} refused by both (programs kept as written, refused on purpose)`,
  ...[...larger, ...smaller, ...changed, ...status].map((l) => `  ${l}`),
  `- compile time, now / before, median of ${String(rounds)} rounds: ${median.toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)})`,
  `- compiler: ${String(sizeBefore)} → ${String(sizeNow)} bytes (${sizeNow >= sizeBefore ? "+" : ""}${String(sizeNow - sizeBefore)})`,
];
console.log(out.join("\n"));
const failed =
  larger.length > 0 || median > 1.05 || status.some((s) => s.endsWith("no longer builds"));
process.exit(failed ? 1 : 0);
