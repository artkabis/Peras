// Decision 0032's protocol: shared values, `Rc<T>`. r1, read only and
// copied on write, chosen, is the build's; r0, no `Rc`, is built when
// PERAS_PROTO_RC names it. r2, changed in place with `Weak<T>`, was measured
// at commit b30819e and removed once not chosen (its runtime cost every
// build of the compiler 90 KB): its figures are in the decision. r3, a
// detector of cycles, was not built.
//
// 1. Ease of writing: the four programs of bench/shared/ease, written as a
//    user writes them from JavaScript, built by each option: the errors met,
//    the lines the fixed version changes, its result against JavaScript's,
//    the blocks it leaves alive on the debug allocator.
// 2. Leaks: P3 as written, its narrowing fixed, built by each option that
//    can build it: the blocks left alive once its last holder ends.
// 3. The workloads of bench/shared/workloads, owned modules on TLSF in Node,
//    every option's module in one process, the order alternating round by
//    round: the median time, its ratio to r0 round by round, the CPU time
//    over the wall time, the module's memory, its bytes.
// 4. The count operations of a frame of W1, by the module's calls to the
//    runtime, its runtime imported (`--link off`) on the debug allocator.
// 5. Whether a turn of `parallel for` may read, hold or write a shared value.
//
// Usage: node scripts/bench-shared.mjs [--check] [--rounds N]
//        --check: one round, the results checked, nothing written.
// Writes bench/shared/RESULTS.md, unless --check.
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { basename, join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "shared");
const work = join(root, "target", "bench-shared");
const perasc = join(root, "target", "release", "perasc");
const OPTIONS = ["r0", "r1"];

rmSync(work, { recursive: true, force: true });

let builds = 0;
/**
 * Builds a source with an option of PERAS_PROTO_RC.
 * @param {string} source
 * @param {string} option
 * @param {string[]} [extra] more arguments: the allocator, `--link off`…
 * @returns {{ ok: true, js: string, bytes: { raw: number, gzip: number } } | { ok: false, errors: string[] }}
 */
function build(source, option, extra = ["--allocator", "tlsf"]) {
  builds += 1;
  const name = basename(source, ".peras");
  const dir = join(work, `${String(builds)}-${name}-${option}`);
  mkdirSync(dir, { recursive: true });
  const args = ["build", source, "--out", dir, "--color", "never", ...extra];
  const r = spawnSync(perasc, args, {
    encoding: "utf8",
    env: { ...process.env, PERAS_PROTO_RC: option },
  });
  if (r.status !== 0) {
    return {
      ok: false,
      errors: r.stderr
        .split("\n")
        .filter((l) => l.startsWith("error["))
        .map((l) => l.slice(6, 11)),
    };
  }
  const js = join(dir, `${name}.js`);
  // The module's memory, read by the measure only.
  const text = readFileSync(js, "utf8").replace(
    "export const __perasHeap = {",
    "export const __perasHeap = {\n  memory: () => $runtime.memory.buffer.byteLength,",
  );
  const measured = join(dir, `${name}.measure.mjs`);
  writeFileSync(measured, text);
  const wasm = readFileSync(join(dir, `${name}.wasm`));
  return { ok: true, js: measured, bytes: { raw: wasm.length, gzip: gzipSync(wasm).length } };
}

/** @typedef {{ view(): Int32Array | Float64Array, release(): void }} Lent */
/** @typedef {Record<string, (...args: unknown[]) => unknown> & { __perasHeap: { live(): number, memory(): number, array(kind: string, n: number): Lent } }} Loaded */

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

/**
 * Builds a source and loads it, or stops the run.
 * @param {string} source
 * @param {string} option
 * @param {string[]} [extra]
 */
async function made(source, option, extra) {
  const built = build(source, option, extra);
  if (!built.ok) throw new Error(`${source}, ${option}: ${built.errors.join(", ")}`);
  return { m: await load(built.js), bytes: built.bytes };
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;

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
  return x.filter((l) => !y.includes(l)).length + y.filter((l) => !x.includes(l)).length;
}

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds.`,
  "",
];

// 1. Ease of writing. Each program's result is what JavaScript gives, but
// P4's, where JavaScript's two holders see the same list: in Peras, as with
// every value, the other holder must not see the change.
/** @type {[string, unknown[], string][]} */
const programs = [
  ["p1", [1000], "1000"],
  ["p2", [[5, 3, 8, 1, 4, 7, 9]], "37"],
  ["p3", [10], "100"],
  ["p4", [], "34"],
];
lines.push("### Ease of writing", "");
lines.push(
  "Each program as a user writes it from JavaScript, built by each option; then its version fixed until it builds and gives JavaScript's result, run on the debug allocator.",
  "",
);
lines.push(
  "| Program | Option | Errors met as written | Fixed version | Lines changed | Result | Left alive |",
);
lines.push("| --- | --- | --- | --- | ---: | --- | ---: |");
for (const [p, input, want] of programs) {
  const natural = join(bench, "ease", `${p}.peras`);
  for (const o of OPTIONS) {
    const first = build(natural, o, ["--allocator", "debug"]);
    const errors = first.ok ? "builds" : first.errors.join(", ");
    const names = [`${p}-${o}`];
    for (const name of names) {
      const fixed = join(bench, "ease", "fixed", `${name}.peras`);
      const file = existsSync(fixed) ? fixed : natural;
      const { m } = await made(file, o, ["--allocator", "debug"]);
      const got = String(m[p]?.(...input));
      const live = m.__perasHeap.live();
      const label = file === natural ? "as written" : `${name}.peras`;
      if (got !== want) throw new Error(`${p} ${o}, ${label}: ${got}, not ${want}`);
      if (live !== 0) {
        throw new Error(`${p} ${o}, ${label}: ${String(live)} blocks alive`);
      }
      lines.push(
        `| ${p} | ${o} | ${errors} | ${label} | ${String(file === natural ? 0 : changed(natural, file))} | ${got} | ${String(live)} |`,
      );
    }
  }
}
lines.push("");
lines.push("Notions JavaScript does not ask for, by option:", "");
lines.push("- r0: a number for each shared value, kept in an array (P1, P2, P3); `copy()` (P4).");
lines.push(
  "- r1: `Rc<T>` and `Rc.new`; a value written through one holder is that holder's own (P4); no cycle can be built, so a child cannot hold the parent it is added to (P3).",
);
lines.push("");

// 2. Leaks: P3 as written, its parent held strongly.
lines.push("### Leaks: P3 as written, on the debug allocator", "");
lines.push("| Option | Builds | Result (JavaScript: 100) | Blocks left alive |");
lines.push("| --- | --- | --- | ---: |");
const p3 = join(bench, "ease", "p3.peras");
for (const o of OPTIONS) {
  const built = build(p3, o, ["--allocator", "debug"]);
  if (!built.ok) {
    lines.push(`| ${o} | no: ${built.errors.join(", ")} | - | - |`);
    continue;
  }
  const m = await load(built.js);
  const got = String(m.p3?.(10));
  const live = m.__perasHeap.live();
  if (o === "r1" && (got !== "45" || live !== 0)) {
    throw new Error(`P3 as written, ${o}: ${got}, ${String(live)} blocks alive`);
  }
  lines.push(`| ${o} | yes | ${got} | ${String(live)} |`);
}
lines.push(
  "",
  "Under r1, each child holds the parent as it was when the child was made, before its own addition: a value can only point to values that existed before it, so the cycle cannot be built, and the result is not JavaScript's.",
  "",
);

// 3. The workloads.
const WIDTH = 1024;
const HEIGHT = 768;
const LANTERNS = 1000;
const FRAME = 600;
/**
 * W1's frame drawn by a module: its pixels' digest, and a function that
 * draws the frame again into the same arrays.
 * @param {Loaded} m
 */
function pond(m) {
  const heap = m.__perasHeap;
  const pixels = heap.array("i32", WIDTH * HEIGHT);
  const depth = heap.array("f64", WIDTH * HEIGHT);
  const light = heap.array("f64", 3 * WIDTH * HEIGHT);
  const draw = () => m.render?.(pixels, depth, light, WIDTH, HEIGHT, LANTERNS, FRAME);
  draw();
  const digest = createHash("sha256")
    .update(Buffer.from(pixels.view().buffer, pixels.view().byteOffset, WIDTH * HEIGHT * 4))
    .digest("hex");
  const release = () => {
    for (const a of [pixels, depth, light]) a.release();
  };
  return { draw, digest, release };
}
const wl = (/** @type {string} */ f) => join(bench, "workloads", f);
const pondR0 = join(root, "bench", "lanterns", "pond.peras");
/** @type {[string, Record<string, string>, (m: Loaded) => { draw: () => unknown, digest: string, release: () => void }][]} */
const workloads = [
  [
    `W1, the pond, frame ${String(FRAME)} at ${String(WIDTH)} × ${String(HEIGHT)} with ${String(LANTERNS)} lanterns`,
    { r0: pondR0, r1: wl("w1-rc.peras") },
    pond,
  ],
  [
    "W2, a tree of 1,000,000 nodes built, summed, then released",
    { r0: wl("w2-index.peras"), r1: wl("w2-rc.peras") },
    (m) => {
      const draw = () => m.tree?.(1_000_000);
      return { draw, digest: String(draw()), release: () => undefined };
    },
  ],
];
for (const [label, files, use] of workloads) {
  /** @type {{ option: string, m: Loaded, bytes: { raw: number, gzip: number }, run: ReturnType<typeof use> }[]} */
  const modules = [];
  for (const option of OPTIONS) {
    const { m, bytes } = await made(files[option] ?? "", option);
    modules.push({ option, m, bytes, run: use(m) });
  }
  const want = modules[0]?.run.digest;
  for (const { option, run } of modules) {
    if (run.digest !== want)
      throw new Error(`${label}, ${option}: ${run.digest}, not ${String(want)}`);
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
      modules[i]?.run.draw();
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[i]?.push(t);
      wall[i] = (wall[i] ?? 0) + t;
      cpu[i] = (cpu[i] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  lines.push(`### ${label}`, "");
  lines.push(
    "| Option | Median | / r0, median (range) | CPU / wall | Memory | Left alive | Bytes (gzip) |",
  );
  lines.push("| --- | ---: | --- | ---: | ---: | ---: | ---: |");
  modules.forEach(({ option, m, bytes, run }, i) => {
    const own = times[i] ?? [];
    const ratios = own.map((t, k) => t / (times[0]?.[k] ?? Number.NaN));
    const memory = m.__perasHeap.memory();
    run.release();
    const live = m.__perasHeap.live();
    if (live !== 0) throw new Error(`${label}, ${option}: ${String(live)} blocks alive`);
    lines.push(
      `| ${option} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[i] ?? 0) / (wall[i] ?? 1)).toFixed(2)} | ${String(memory / 1024)} KiB | ${String(live)} | ${String(bytes.raw)} (${String(bytes.gzip)}) |`,
    );
  });
  lines.push("");
}

// 4. The count operations of a frame of W1: each call the module makes to
// the runtime's `buf_clone` or `buf_release` on a shared value, or a weak
// holder of one, its runtime imported so that its calls can be seen.
lines.push("### Count operations in a frame of W1", "");
lines.push(
  "| Option | Holders taken | Holders let go | When the lanterns are made | In the loop that draws |",
);
lines.push("| --- | ---: | ---: | ---: | ---: |");
for (const option of ["r1"]) {
  const built = build(wl("w1-rc.peras"), option, ["--allocator", "debug", "--link", "off"]);
  if (!built.ok) throw new Error(`W1 unlinked, ${option}: ${built.errors.join(", ")}`);
  const text = readFileSync(built.js, "utf8");
  const counted = text
    .replace(
      "const $imports = {",
      "globalThis.$rc = { taken: 0, let: 0 };\nconst $isRc = (h) => (h & 1) !== 0 || (new Int32Array($runtime.memory.buffer)[(h >>> 2) + 3] & 0x4000) !== 0;\nconst $imports = {",
    )
    .replace(
      "buf_release: $runtime.buf_release,",
      "buf_release: (h, s) => { if ($isRc(h)) globalThis.$rc.let++; $runtime.buf_release(h, s); },",
    )
    .replace(
      "buf_clone: $runtime.buf_clone,",
      "buf_clone: (h, s) => { if ($isRc(h)) globalThis.$rc.taken++; return $runtime.buf_clone(h, s); },",
    );
  if (counted === text || !counted.includes("$rc.taken++") || !counted.includes("$rc.let++")) {
    throw new Error("W1 unlinked: the runtime's imports were not found");
  }
  const file = built.js.replace(/\.measure\.mjs$/, ".counted.mjs");
  writeFileSync(file, counted);
  const m = await load(file);
  const run = pond(m);
  const counts = /** @type {{ $rc: { taken: number, let: number } }} */ (
    /** @type {unknown} */ (globalThis)
  ).$rc;
  counts.taken = 0;
  counts.let = 0;
  run.draw();
  const { taken, let: gone } = counts;
  run.release();
  // A lantern takes a holder of the mesh when it is made, and `shared`, the
  // mesh's first holder, lets it go at the end; the array of lanterns lets
  // theirs go inside the runtime, unseen here. The rest is the loop's.
  const loop = taken + gone - LANTERNS - 1;
  lines.push(
    `| ${option} | ${String(taken)} | ${String(gone)} | ${String(LANTERNS)} | ${String(loop)} |`,
  );
  if (check && loop !== 2 * LANTERNS && loop !== 0) {
    throw new Error(`W1 ${option}: ${String(loop)} count operations in the loop that draws`);
  }
}
lines.push(
  "",
  "Counted by the module's calls to its runtime: the 1,000 holders the array of lanterns lets go when it is released, inside the runtime, are not seen here; the one `shared`, the mesh's first holder, lets go at the end is counted, and set apart from the loop's.",
  "",
);

// 5. Parallel loops.
lines.push("### Shared values in a turn of `parallel for`", "");
lines.push("| Turn | r1 |");
lines.push("| --- | --- |");
for (const [label, file, refused] of /** @type {[string, string, boolean[]][]} */ ([
  ["reads a shared value", "reads.peras", [false]],
  ["holds it once more", "holds.peras", [true]],
  ["writes a field of it", "writes.peras", [true]],
])) {
  const cells = ["r1"].map((option, i) => {
    const built = build(join(bench, "parallel", file), option, ["--threads", "on"]);
    if (built.ok === refused[i]) {
      throw new Error(`parallel ${file}, ${option}: ${built.ok ? "accepted" : "refused"}`);
    }
    return built.ok ? "accepted" : `refused, ${built.errors.join(", ")}`;
  });
  lines.push(`| ${label} | ${cells.join(" | ")} |`);
}
lines.push("");

// 6. Two holders of one value: a loop over a field of one, while the other
// replaces that field.
lines.push("### A loop over a field another holder replaces", "");
lines.push("| Option | Result (JavaScript: 6) | Left alive |");
lines.push("| --- | ---: | ---: |");
for (const [option, want] of [["r1", "6"]]) {
  const { m } = await made(join(bench, "aliasing", "loop.peras"), option ?? "", [
    "--allocator",
    "debug",
  ]);
  const got = String(m.alias?.());
  const live = m.__perasHeap.live();
  if (got !== want || live !== 0)
    throw new Error(`aliasing, ${String(option)}: ${got}, ${String(live)}`);
  lines.push(`| ${String(option)} | ${got} | ${String(live)} |`);
}
lines.push("", "r3, a detector of cycles at release, is not built: see the decision.", "");

console.log(lines.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "RESULTS.md"),
    `# Shared values, \`Rc<T>\`: the options measured\n\nGenerated by \`node scripts/bench-shared.mjs\`, for [decision 0032](../../docs/en/decisions/0032-shared-values.md).\n\n${lines.join("\n")}`,
  );
}
