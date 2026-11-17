// Decision 0036's protocol: small functions inlined (i1), wider invariants
// (i2), both (i3), against today (i0), each built by PERAS_PROTO_INLINE.
//
// 2. Speed and load on the processor: owned modules on TLSF in Node, every
//    option's module in one process, the order alternating round by round;
//    the median time, its ratio to i0 round by round and their range, the CPU
//    time over the wall time.
//    W1, the pond, frame 600 at 1024 × 768 with 1,000 lanterns;
//    W2, the garden of decision 0013, one frame at 1024 × 1024;
//    W3, each kernel of bench/browser/kernels.peras.
// 3. Bytes: each workload's module, raw and gzipped; every module of the
//    repository is compared by scripts/check-regression.mjs.
//
// Usage: node scripts/bench-inline.mjs [--check] [--rounds N] [--options i0,i1]
//        --check: one round, smaller workloads, every option's results
//        checked against i0's, nothing written.
// Writes bench/inline/RESULTS.md, unless --check.
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";

const check = process.argv.includes("--check");
const arg = (/** @type {string} */ name) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : undefined;
};
const ROUNDS = check ? 1 : Number(arg("--rounds") ?? 21);
const OPTIONS = (arg("--options") ?? "i0,i1,i2,i3").split(",");
const root = join(import.meta.dirname, "..");
const work = join(root, "target", "bench-inline");
const perasc = join(root, "target", "release", "perasc");
const scale = check ? 0.1 : 1;

rmSync(work, { recursive: true, force: true });

/** @typedef {Record<string, (...args: unknown[]) => unknown> & { __perasHeap: { array(kind: string, n: number): { view(): ArrayLike<number> & { set(from: ArrayLike<number>): void, fill(v: number): void, buffer: ArrayBufferLike, byteOffset: number, byteLength: number }, release(): void } } }} Loaded */

/**
 * Builds a source as an owned module on TLSF under an option, and loads it.
 * @param {string} source
 * @param {string} option
 */
async function made(source, option) {
  const name = source.split("/").pop()?.replace(".peras", "") ?? "m";
  const dir = join(work, `${name}-${option}`);
  mkdirSync(dir, { recursive: true });
  const r = spawnSync(
    perasc,
    ["build", source, "--out", dir, "--allocator", "tlsf", "--color", "never"],
    { encoding: "utf8", env: { ...process.env, PERAS_PROTO_INLINE: option } },
  );
  if (r.status !== 0) throw new Error(`${source}, ${option}: ${r.stderr}`);
  const wasm = readFileSync(join(dir, `${name}.wasm`));
  /** @type {unknown} */
  const m = await import(join(dir, `${name}.js`));
  return {
    m: /** @type {Loaded} */ (m),
    bytes: { raw: wasm.length, gzip: gzipSync(wasm).length },
  };
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;
/** @param {ArrayBufferView} v */
const sha = (v) =>
  createHash("sha256")
    .update(Buffer.from(v.buffer, v.byteOffset, v.byteLength))
    .digest("hex");

/**
 * A workload: given a loaded module, gives the timed call, the untimed
 * preparation before it, and a digest of what it computed.
 * @typedef {{ prepare: () => void, run: () => unknown, digest: () => string }} Run
 * @typedef {[string, string, (m: Loaded) => Run]} Workload
 */
/** A workload with nothing to prepare. */
const nothing = () => undefined;
const n = (/** @type {number} */ x) => Math.max(1, Math.round(x * scale));
const kernels = join(root, "bench", "browser", "kernels.peras");
/** A pseudo-random sequence, the same for every module. @param {number} count */
function noise(count) {
  const out = new Float64Array(count);
  let s = 12345;
  for (let i = 0; i < count; i++) {
    s = (Math.imul(s, 1103515245) + 12345) >>> 0;
    out[i] = s / 4294967296;
  }
  return out;
}
/** @type {Workload[]} */
const workloads = [
  [
    "W1, the pond, frame 600 at 1024 × 768 with 1,000 lanterns",
    join(root, "bench", "lanterns", "pond.peras"),
    (m) => {
      const [w, h] = check ? [256, 192] : [1024, 768];
      const pixels = m.__perasHeap.array("i32", w * h);
      const depth = m.__perasHeap.array("f64", w * h);
      const light = m.__perasHeap.array("f64", 3 * w * h);
      let filled = 0;
      return {
        prepare: nothing,
        run: () => (filled = Number(m.render?.(pixels, depth, light, w, h, 1000, 600))),
        digest: () => `${String(filled)} ${sha(pixels.view())}`,
      };
    },
  ],
  [
    "W2, the garden, one frame at 1024 × 1024",
    join(root, "bench", "garden", "garden.peras"),
    (m) => {
      const side = check ? 256 : 1024;
      const pixels = m.__perasHeap.array("i32", side * side);
      return {
        prepare: nothing,
        run: () => m.render?.(pixels, side, side, 1, 0.5, 0.5),
        digest: () => sha(pixels.view()),
      };
    },
  ],
  [
    "W3, particles: 1,000,000 particles, 10 steps",
    kernels,
    (m) => {
      const count = n(1_000_000);
      const xs = m.__perasHeap.array("f64", count);
      const vs = m.__perasHeap.array("f64", count);
      const x0 = noise(count);
      let result = 0;
      return {
        prepare: () => {
          xs.view().set(x0);
          vs.view().fill(0.5);
        },
        run: () => (result = Number(m.particles?.(xs, vs, 10))),
        digest: () => `${String(result)} ${sha(xs.view())}`,
      };
    },
  ],
  [
    "W3, grayscale: 2048 × 2048 pixels",
    kernels,
    (m) => {
      const count = n(2048 * 2048);
      const pixels = m.__perasHeap.array("i32", count);
      const p0 = Int32Array.from(noise(count), (x) => Math.floor(x * 4294967296) | 0);
      let result = 0;
      return {
        prepare: () => {
          pixels.view().set(p0);
        },
        run: () => (result = Number(m.grayscale?.(pixels))),
        digest: () => `${String(result)} ${sha(pixels.view())}`,
      };
    },
  ],
  [
    "W3, nbody: 1,000 bodies, 5 steps",
    kernels,
    (m) => {
      let result = 0;
      return {
        prepare: nothing,
        run: () => (result = Number(m.nbody?.(n(1000), 5))),
        digest: () => String(result),
      };
    },
  ],
  [
    "W3, hash64: 2,000,000 rounds",
    kernels,
    (m) => {
      let result = "";
      return {
        prepare: nothing,
        run: () => (result = String(m.hash64?.(n(2_000_000)))),
        digest: () => result,
      };
    },
  ],
  [
    "W3, sorted: 1,000,000 floats",
    kernels,
    (m) => {
      const count = n(1_000_000);
      const xs = m.__perasHeap.array("f64", count);
      const x0 = noise(count);
      let result = 0;
      return {
        prepare: () => {
          xs.view().set(x0);
        },
        run: () => (result = Number(m.sorted?.(xs))),
        digest: () => `${String(result)} ${sha(xs.view())}`,
      };
    },
  ],
  [
    "W3, mandel: 600 × 600",
    kernels,
    (m) => {
      let result = 0;
      return {
        prepare: nothing,
        run: () => (result = Number(m.mandel?.(n(600)))),
        digest: () => String(result),
      };
    },
  ],
  [
    "W3, words: 200,000 words",
    kernels,
    (m) => {
      const text = Array.from({ length: n(200_000) }, (_, i) =>
        i % 7 === 0 ? "lantern" : "pond",
      ).join(" ");
      let result = 0;
      return {
        prepare: nothing,
        run: () => (result = Number(m.words?.(text))),
        digest: () => String(result),
      };
    },
  ],
];

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; options ${OPTIONS.join(", ")}.`,
  "",
];
/** @type {Map<string, Promise<{ m: Loaded, bytes: { raw: number, gzip: number } }>>} */
const loaded = new Map();
for (const [label, source, use] of workloads) {
  const modules = [];
  for (const option of OPTIONS) {
    const key = `${source} ${option}`;
    if (!loaded.has(key)) loaded.set(key, made(source, option));
    const built =
      await /** @type {Promise<{ m: Loaded, bytes: { raw: number, gzip: number } }>} */ (
        loaded.get(key)
      );
    const run = use(built.m);
    run.prepare();
    run.run();
    modules.push({ option, bytes: built.bytes, run, first: run.digest() });
  }
  const want = modules[0]?.first;
  for (const { option, first } of modules) {
    if (first !== want) throw new Error(`${label}, ${option}: ${first}, not ${String(want)}`);
  }
  const times = modules.map(() => /** @type {number[]} */ ([]));
  const cpu = modules.map(() => 0);
  const wall = modules.map(() => 0);
  for (let round = 0; round < ROUNDS; round++) {
    const order = modules.map((_, i) => i);
    if (round % 2 === 1) order.reverse();
    for (const i of order) {
      const run = modules[i]?.run;
      run?.prepare();
      const c0 = process.cpuUsage();
      const t0 = performance.now();
      run?.run();
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[i]?.push(t);
      wall[i] = (wall[i] ?? 0) + t;
      cpu[i] = (cpu[i] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  lines.push(`### ${label}`, "");
  lines.push("| Option | Median | / i0, median (range) | CPU / wall | Bytes (gzip) |");
  lines.push("| --- | ---: | --- | ---: | ---: |");
  modules.forEach(({ option, bytes }, i) => {
    const own = times[i] ?? [];
    const ratios = own.map((t, k) => t / (times[0]?.[k] ?? Number.NaN));
    lines.push(
      `| ${option} | ${median(own).toFixed(2)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[i] ?? 0) / (wall[i] ?? 1)).toFixed(2)} | ${String(bytes.raw)} (${String(bytes.gzip)}) |`,
    );
  });
  lines.push("");
}

console.log(lines.join("\n"));
if (!check) {
  mkdirSync(join(root, "bench", "inline"), { recursive: true });
  writeFileSync(
    join(root, "bench", "inline", "RESULTS.md"),
    `# Small functions inlined, invariants moved out of branches: the options measured\n\nGenerated by \`node scripts/bench-inline.mjs\`, for [decision 0036](../../docs/en/decisions/0036-inlining-and-invariants.md).\n\n${lines.join("\n")}`,
  );
}
