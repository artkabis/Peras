// Decision 0028's protocol: taking a value or leaving, and when a move
// happens. Each option is a prototype the compiler builds when
// PERAS_PROTO_MOVE (Q1), PERAS_PROTO_TAKE (Q2) or PERAS_PROTO_FALLBACK (Q3)
// names it; 2e, narrowing, was found by measuring the others.
//
// 1. The workloads of bench/options/workloads, built by each option and as a
//    managed module (JavaScript), checked to give the same result, then run in
//    one process, the order alternating round by round: the median time, its
//    ratio to the option that keeps today's behaviour round by round, the CPU
//    time over the wall time, the module's memory, the blocks left alive, the
//    bytes.
// 2. The six programs of bench/options/ease, written as a user would from
//    JavaScript: the errors each option meets, the lines its fixed version
//    changes, its result checked against the managed module.
// 3. Question 3's agreement of the modes: a fallback with an effect.
//
// Usage: node scripts/bench-options.mjs [--check] [--rounds N]
//        --check: one round, the results checked, nothing written.
// Writes bench/options/RESULTS.md, unless --check.
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
const bench = join(root, "bench", "options");
const work = join(root, "target", "bench-options");
const perasc = join(root, "target", "release", "perasc");

rmSync(work, { recursive: true, force: true });

let builds = 0;
/**
 * Builds a source with the prototype variables given, owned or managed.
 * @param {string} source
 * @param {Record<string, string>} env
 * @param {boolean} managed
 * @returns {{ ok: true, js: string, bytes: { raw: number, gzip: number } } | { ok: false, errors: string[] }}
 */
function build(source, env, managed) {
  builds += 1;
  const dir = join(work, `${String(builds)}-${basename(source, ".peras")}`);
  mkdirSync(dir, { recursive: true });
  let file = source;
  if (managed) {
    file = join(dir, `${basename(source, ".peras")}.apeiron`);
    copyFileSync(source, file);
  }
  const args = ["build", file, "--out", dir, "--color", "never"];
  if (!managed) args.push("--allocator", "tlsf");
  const r = spawnSync(perasc, args, { encoding: "utf8", env: { ...process.env, ...env } });
  if (r.status !== 0) {
    return { ok: false, errors: r.stderr.split("\n").filter((l) => l.startsWith("error[")) };
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
  const shipped = managed ? Buffer.from(text) : readFileSync(join(dir, `${name}.wasm`));
  return { ok: true, js: measured, bytes: { raw: shipped.length, gzip: gzipSync(shipped).length } };
}

/** @typedef {Record<string, (...args: unknown[]) => unknown> & { __perasHeap?: { live(): number, memory(): number } }} Loaded */

/**
 * Loads a built module.
 * @param {string} js
 * @returns {Promise<Loaded>}
 */
async function load(js) {
  /** @type {unknown} */
  const loaded = await import(js);
  return /** @type {Loaded} */ (loaded);
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;

/**
 * An option of a workload: its label, its source, its variables.
 * @typedef {{ label: string, file: string, env: Record<string, string> }} Option
 */
const w = (/** @type {string} */ f) => join(bench, "workloads", f);
/** @type {[string, string, number[], Option[], Option][]} */
const workloads = [
  [
    "W1, P1 over 1,000,000 words",
    "w1",
    [1_000_000],
    [
      { label: "1a, copied by the program", file: w("w1-copy.peras"), env: {} },
      { label: "1b, moved at the call", file: w("w1-count.peras"), env: { PERAS_PROTO_MOVE: "b" } },
      {
        label: "1c, copied for the program",
        file: w("w1-count.peras"),
        env: { PERAS_PROTO_MOVE: "c" },
      },
    ],
    { label: "managed", file: w("w1-copy.peras"), env: {} },
  ],
  [
    "W2, P2 over 1,000,000 lines, one in ten bad",
    "w2",
    [1_000_000],
    [
      { label: "2a, a test and a placeholder", file: w("w2-a.peras"), env: {} },
      { label: "2b, an arm that leaves", file: w("w2-b.peras"), env: { PERAS_PROTO_TAKE: "b" } },
      { label: "2c, const … else", file: w("w2-c.peras"), env: { PERAS_PROTO_TAKE: "c" } },
      { label: "2e, narrowed", file: w("w2-e.peras"), env: { PERAS_PROTO_TAKE: "e" } },
    ],
    { label: "managed", file: w("w2-a.peras"), env: {} },
  ],
  [
    "W2, every line good: unwrap()",
    "w2",
    [1_000_000],
    [
      { label: "2a, a test and a placeholder", file: w("w2-d-ref.peras"), env: {} },
      { label: "2d, unwrap()", file: w("w2-d.peras"), env: { PERAS_PROTO_TAKE: "d" } },
    ],
    { label: "managed", file: w("w2-d-ref.peras"), env: {} },
  ],
  [
    "W3, P4 over 1,000,000 lookups, nine in ten found",
    "w3",
    [1_000_000],
    [
      { label: "3a, always", file: w("w3.peras"), env: { PERAS_PROTO_FALLBACK: "a" } },
      { label: "3b, only when needed", file: w("w3.peras"), env: { PERAS_PROTO_FALLBACK: "b" } },
      { label: "3c, unwrapOrElse", file: w("w3-c.peras"), env: { PERAS_PROTO_FALLBACK: "c" } },
    ],
    { label: "managed", file: w("w3.peras"), env: { PERAS_PROTO_FALLBACK: "b" } },
  ],
];

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; owned modules on TLSF.`,
  "",
];
for (const [label, fn, args, options, reference] of workloads) {
  const all = [...options, reference];
  /** @type {{ option: Option, m: Loaded, bytes: { raw: number, gzip: number } }[]} */
  const modules = [];
  for (const option of all) {
    const built = build(option.file, option.env, option === reference);
    if (!built.ok) throw new Error(`${label}, ${option.label}: ${built.errors.join(" | ")}`);
    modules.push({ option, m: await load(built.js), bytes: built.bytes });
  }
  const small = args.map(() => 1000);
  for (const run of [small, args]) {
    const want = modules.at(-1)?.m[fn]?.(...run);
    for (const { option, m } of modules) {
      const got = m[fn]?.(...run);
      if (got !== want)
        throw new Error(`${label}, ${option.label}: ${String(got)}, not ${String(want)}`);
    }
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
      modules[i]?.m[fn]?.(...args);
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[i]?.push(t);
      wall[i] = (wall[i] ?? 0) + t;
      cpu[i] = (cpu[i] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  lines.push(`### ${label}`, "");
  lines.push(
    "| Option | Median | / first, median (range) | CPU / wall | Memory | Left alive | Bytes (gzip) |",
  );
  lines.push("| --- | ---: | --- | ---: | ---: | ---: | ---: |");
  modules.forEach(({ option, m, bytes }, i) => {
    const own = times[i] ?? [];
    const ratios = own.map((t, k) => t / (times[0]?.[k] ?? Number.NaN));
    const heap = m.__perasHeap;
    if (heap && heap.live() !== 0)
      throw new Error(`${label}, ${option.label}: ${String(heap.live())} blocks alive`);
    lines.push(
      `| ${option.label} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[i] ?? 0) / (wall[i] ?? 1)).toFixed(2)} | ${heap ? `${String(heap.memory() / 1024)} KiB` : "-"} | ${heap ? String(heap.live()) : "-"} | ${String(bytes.raw)} (${String(bytes.gzip)}) |`,
    );
  });
  lines.push("");
}

// Ease of writing: the errors met by the program as a user writes it, and
// the lines its fixed version changes.
/** @type {[string, unknown[], string, string[]][]} */
const programs = [
  ["p1", ["a b a c b a"], "PERAS_PROTO_MOVE", ["a", "b", "c"]],
  ["p6", ["a,1\nb,2,3\na,4"], "PERAS_PROTO_MOVE", ["a", "b", "c"]],
  ["p2", ["a,1\nbad\nb,22"], "PERAS_PROTO_TAKE", ["a", "b", "c", "d", "e"]],
  ["p3", ["tea"], "PERAS_PROTO_TAKE", ["a", "b", "c", "d", "e"]],
  ["p5", [24], "PERAS_PROTO_TAKE", ["a", "b", "c", "d", "e"]],
  ["p4", [7], "PERAS_PROTO_FALLBACK", ["a", "b", "c"]],
];
/**
 * The lines two sources differ by, comments aside: removed plus added.
 * @param {string} a
 * @param {string} b
 */
function changed(a, b) {
  const code = (/** @type {string} */ f) =>
    readFileSync(f, "utf8")
      .split("\n")
      .filter((l) => !l.trimStart().startsWith("//"));
  const x = code(a);
  const y = code(b);
  const removed = x.filter((l) => !y.includes(l)).length;
  const added = y.filter((l) => !x.includes(l)).length;
  return removed + added;
}
lines.push("### Ease of writing", "");
lines.push("| Program | Option | Errors met as written | Lines changed to build |");
lines.push("| --- | --- | --- | ---: |");
for (const [p, input, variable, options] of programs) {
  const natural = join(bench, "ease", `${p}.peras`);
  /** @type {string | undefined} */
  let want;
  for (const o of options) {
    const env = { [variable]: o };
    const first = build(natural, env, false);
    const errors = first.ok ? [] : first.errors.map((e) => e.slice(6, 11));
    const fixedFile = join(bench, "ease", "fixed", `${p}-${o}.peras`);
    const file = existsSync(fixedFile) && !first.ok ? fixedFile : natural;
    const owned = build(file, env, false);
    const managed = build(file, env, true);
    if (!owned.ok || !managed.ok) throw new Error(`${p} ${o}: the fixed program does not build`);
    const om = await load(owned.js);
    const mm = await load(managed.js);
    const got = JSON.stringify(om[p]?.(...input));
    const reference = JSON.stringify(mm[p]?.(...input));
    want ??= reference;
    if (got !== reference || got !== want || (om.__perasHeap?.live() ?? 0) !== 0) {
      throw new Error(`${p} ${o}: ${got}, managed ${reference}, first ${want}`);
    }
    const lines_ = file === natural ? 0 : changed(natural, file);
    lines.push(
      `| ${p} | ${o} | ${errors.length === 0 ? "builds" : errors.join(", ")} | ${String(lines_)} |`,
    );
  }
}
lines.push("");

// Question 3: a fallback with an effect, in both modes.
const effect = join(bench, "workloads", "q3-effect.peras");
lines.push("### Question 3: a fallback with an effect", "");
lines.push("| Option | Owned | Managed | Agree |");
lines.push("| --- | --- | --- | --- |");
for (const o of ["today", "a", "b", "c"]) {
  const env = o === "today" ? {} : { PERAS_PROTO_FALLBACK: o };
  const owned = build(effect, env, false);
  const managed = build(effect, env, true);
  if (!owned.ok || !managed.ok) throw new Error(`q3 ${o}: does not build`);
  const om = await load(owned.js);
  const mm = await load(managed.js);
  const a = `${String(om.effects?.(5))}, ${String(om.effects?.(0))}`;
  const b = `${String(mm.effects?.(5))}, ${String(mm.effects?.(0))}`;
  lines.push(`| ${o} | ${a} | ${b} | ${a === b ? "yes" : "no"} |`);
}
lines.push("");

console.log(lines.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# Taking a value or leaving, and when a move happens: the options measured\n\nGenerated by \`node scripts/bench-options.mjs\`, for [decision 0028](../../docs/en/decisions/0028-options-and-moves.md).\n\n${lines.join("\n")}`,
  );
}
