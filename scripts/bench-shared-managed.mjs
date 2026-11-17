// Decision 0033's protocol: shared values in a managed module. Option m3,
// chosen, a box counted and never released, is the build's; m1 (copied at
// every write) and m2 (copied at every holder) were measured at commit
// 86de5c1 and removed once not chosen: their figures are in the decision.
//
// 1. Agreement: every function of the end-to-end fixture of `Rc`, and P1 to
//    P4 of decision 0032 as fixed for r1, built as a managed module, give
//    what the owned module gives.
// 2. The workloads W1, W2 and W3: the managed module and the owned module
//    of r1 on TLSF in one process, the order alternating round by round:
//    the median time, its ratio to the owned module round by round, the CPU
//    time over the wall time, the bytes.
//
// Usage: node scripts/bench-shared-managed.mjs [--check] [--rounds N]
//        --check: one round, the agreement checked, nothing written.
// Writes bench/shared/MANAGED.md, unless --check.
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { basename, join } from "node:path";
import { performance } from "node:perf_hooks";
import { setFlagsFromString } from "node:v8";
import { runInNewContext } from "node:vm";
import { gzipSync } from "node:zlib";

setFlagsFromString("--expose_gc");
/** @type {unknown} */
const gc = runInNewContext("gc");
const collect = /** @type {() => void} */ (gc);

const check = process.argv.includes("--check");
const roundsAt = process.argv.indexOf("--rounds");
const ROUNDS = check ? 1 : Number(roundsAt > 0 ? process.argv[roundsAt + 1] : 21);
const root = join(import.meta.dirname, "..");
const bench = join(root, "bench", "shared");
const work = join(root, "target", "bench-shared-managed");
const perasc = join(root, "target", "release", "perasc");
const OPTIONS = ["m3"];

rmSync(work, { recursive: true, force: true });

let builds = 0;
/**
 * Builds a source as an owned module on TLSF (`option` "owned"), or as a
 * managed module by an option of PERAS_PROTO_RCM.
 * @param {string} source
 * @param {string} option
 * @returns {{ ok: true, js: string, bytes: { raw: number, gzip: number } } | { ok: false, errors: string[] }}
 */
function build(source, option) {
  builds += 1;
  const name = basename(source, ".peras");
  const dir = join(work, `${String(builds)}-${name}-${option}`);
  mkdirSync(dir, { recursive: true });
  const managed = option !== "owned";
  let file = source;
  if (managed) {
    // A managed module moves nothing, so it takes no `sink` (E0243): the
    // same source without the convention.
    file = join(dir, `${name}.apeiron`);
    copyFileSync(source, file);
    writeFileSync(file, readFileSync(file, "utf8").replaceAll(": sink ", ": "));
  }
  const args = ["build", file, "--out", dir, "--color", "never"];
  if (!managed) args.push("--allocator", "tlsf");
  const r = spawnSync(perasc, args, {
    encoding: "utf8",
    env: process.env,
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
  const shipped = managed ? readFileSync(js) : readFileSync(join(dir, `${name}.wasm`));
  return { ok: true, js, bytes: { raw: shipped.length, gzip: gzipSync(shipped).length } };
}

/** @typedef {Record<string, (...args: unknown[]) => unknown> & { __perasHeap?: { live(): number, array(kind: string, n: number): { view(): Int32Array | Float64Array, release(): void } } }} Loaded */

/**
 * Builds and loads a source, or stops the run.
 * @param {string} source
 * @param {string} option
 */
async function made(source, option) {
  const built = build(source, option);
  if (!built.ok) throw new Error(`${source}, ${option}: ${built.errors.join(", ")}`);
  /** @type {unknown} */
  const m = await import(built.js);
  return { m: /** @type {Loaded} */ (m), bytes: built.bytes };
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds.`,
  "",
];

// 1 and 2. Agreement and ease of writing.
const fixture = join(
  root,
  "packages",
  "vite-plugin-peras",
  "e2e",
  "fixtures",
  "shared",
  "shared.peras",
);
const ease = join(bench, "ease");
/** @type {[string, string, unknown[][]][]} */
const programs = [
  [
    "the end-to-end fixture",
    fixture,
    [
      ["lent"],
      ["reassigned"],
      ["index"],
      ["replaced"],
      ["moved"],
      ["holders"],
      ["tree", 100],
      ["nested"],
      ["inner"],
      ["elements"],
      ["elementFields"],
      ["stepped"],
    ],
  ],
  ["P1", join(ease, "p1.peras"), [["p1", 1000]]],
  ["P2", join(ease, "fixed", "p2-r1.peras"), [["p2", [5, 3, 8, 1, 4, 7, 9]]]],
  ["P3", join(ease, "fixed", "p3-r1.peras"), [["p3", 10]]],
  ["P4", join(ease, "p4.peras"), [["p4"]]],
];
lines.push("### Agreement with the owned module, and ease of writing", "");
lines.push("| Program | Managed module |");
lines.push("| --- | --- |");
for (const [label, source, calls] of programs) {
  const { m: owned } = await made(source, "owned");
  const want = calls.map(([f, ...a]) =>
    String(owned[String(f)]?.(...globalThis.structuredClone(a))),
  );
  const cells = [];
  for (const option of OPTIONS) {
    const { m } = await made(source, option);
    const got = calls.map(([f, ...a]) => String(m[String(f)]?.(...globalThis.structuredClone(a))));
    const agree = got.every((g, i) => g === want[i]);
    if (!agree) throw new Error(`${label}, ${option}: ${got.join(" ")}, owned ${want.join(" ")}`);
    cells.push("builds, agrees");
  }
  lines.push(`| ${label} | ${cells.join(" | ")} |`);
}
lines.push("", "Every program is the one written for the owned module: no line changes.", "");

// 3. The workloads.
const WIDTH = 1024;
const HEIGHT = 768;
const LANTERNS = 1000;
const FRAME = 600;
const wl = (/** @type {string} */ f) => join(bench, "workloads", f);
/**
 * W1's frame drawn by a module, owned or managed.
 * @param {Loaded} m
 */
function pond(m) {
  const heap = m.__perasHeap;
  const owned = heap?.array !== undefined;
  const pixels = owned ? heap.array("i32", WIDTH * HEIGHT) : new Int32Array(WIDTH * HEIGHT);
  const depth = owned ? heap.array("f64", WIDTH * HEIGHT) : new Float64Array(WIDTH * HEIGHT);
  const light = owned
    ? heap.array("f64", 3 * WIDTH * HEIGHT)
    : new Float64Array(3 * WIDTH * HEIGHT);
  const draw = () => m.render?.(pixels, depth, light, WIDTH, HEIGHT, LANTERNS, FRAME);
  draw();
  const view =
    /** @type {{ view?: () => Int32Array }} */ (pixels).view?.() ??
    /** @type {Int32Array} */ (pixels);
  const digest = createHash("sha256")
    .update(Buffer.from(view.buffer, view.byteOffset, WIDTH * HEIGHT * 4))
    .digest("hex");
  return { draw, digest };
}
/** @type {[string, string, (m: Loaded) => { draw: () => unknown, digest: string }][]} */
const workloads = [
  [
    `W1, the pond, frame ${String(FRAME)} at ${String(WIDTH)} × ${String(HEIGHT)} with ${String(LANTERNS)} lanterns`,
    wl("w1-rc.peras"),
    pond,
  ],
  [
    "W2, a tree of 1,000,000 nodes built and summed",
    wl("w2-rc.peras"),
    (m) => {
      const draw = () => m.tree?.(1_000_000);
      return { draw, digest: String(draw()) };
    },
  ],
  [
    "W3, 1,000,000 writes through one of two holders",
    wl("w3-writes.peras"),
    (m) => {
      const draw = () => m.writes?.(1_000_000);
      return { draw, digest: String(draw()) };
    },
  ],
];
for (const [label, source, use] of workloads) {
  /** @type {{ option: string, bytes: { raw: number, gzip: number }, run: ReturnType<typeof use> }[]} */
  const modules = [];
  for (const option of ["owned", ...OPTIONS]) {
    const { m, bytes } = await made(source, option);
    modules.push({ option, bytes, run: use(m) });
  }
  const want = modules[0]?.run.digest;
  for (const { option, run } of modules) {
    if (run.digest !== want)
      throw new Error(`${label}, ${option}: ${run.digest}, not ${String(want)}`);
  }
  const times = modules.map(() => /** @type {number[]} */ ([]));
  const cpu = modules.map(() => 0);
  const wall = modules.map(() => 0);
  const heap = modules.map(() => 0);
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
      collect();
      heap[i] = Math.max(heap[i] ?? 0, process.memoryUsage().heapUsed);
    }
  }
  lines.push(`### ${label}`, "");
  lines.push(
    "| Module | Median | / owned, median (range) | CPU / wall | JavaScript heap after | Bytes (gzip) |",
  );
  lines.push("| --- | ---: | --- | ---: | ---: | ---: |");
  modules.forEach(({ option, bytes }, i) => {
    const own = times[i] ?? [];
    const ratios = own.map((t, k) => t / (times[0]?.[k] ?? Number.NaN));
    lines.push(
      `| ${option === "owned" ? "owned, r1" : `managed, ${option}`} | ${median(own).toFixed(1)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[i] ?? 0) / (wall[i] ?? 1)).toFixed(2)} | ${String(Math.round((heap[i] ?? 0) / 1024))} KiB | ${String(bytes.raw)} (${String(bytes.gzip)}) |`,
    );
  });
  lines.push("");
}

console.log(lines.join("\n"));
if (!check) {
  writeFileSync(
    join(bench, "MANAGED.md"),
    `# Shared values in a managed module: the options measured\n\nGenerated by \`node scripts/bench-shared-managed.mjs\`, for [decision 0033](../../docs/en/decisions/0033-shared-values-in-managed-modules.md).\n\n${lines.join("\n")}`,
  );
}
