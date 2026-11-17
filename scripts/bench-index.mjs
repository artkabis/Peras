// Decision 0037: how a managed module checks its indices. Option o1, chosen,
// the check written at each access, is the build's; o0 (`$at` and `$put`),
// o2 (a pair of functions per kind of element) and o3 (no check) were
// measured at commit 338eceb and removed once not chosen: their figures are
// in the decision. Every module is a production build (`--allocator tlsf`,
// finding M029).
//
// 2. Speed and load on the processor, in Node: the reference written by hand
//    (TypeScript or JavaScript) and every option's module in one process, the
//    order alternating round by round; the median time, its ratio to the
//    reference round by round and their range, the CPU time over the wall
//    time.
//    W1, the pond, frame 600 at 1024 × 768 with 1,000 lanterns, against pond.ts;
//    W2, the garden as a managed module, one frame at 1024 × 1024, against
//        garden.ts;
//    W3, each kernel of bench/managed/kernels.apeiron, against the JavaScript
//        written by hand of scripts/bench-managed.mjs, copied here.
// 3. Bytes: each workload's module, raw and gzipped; every module of the
//    repository is compared by scripts/check-regression.mjs.
//
// Usage: node scripts/bench-index.mjs [--check] [--rounds N]
//        --check: one round, smaller workloads, every option's results
//        checked against the reference's, nothing written.
// Writes bench/index/CURRENT.md, unless --check: RESULTS.md keeps the four
// options as decision 0037 measured them.
import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";
import ts from "typescript";

const check = process.argv.includes("--check");
const arg = (/** @type {string} */ name) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : undefined;
};
const ROUNDS = check ? 1 : Number(arg("--rounds") ?? 21);
const OPTIONS = ["o1"];
const root = join(import.meta.dirname, "..");
const work = join(root, "target", "bench-index");
const perasc = join(root, "target", "release", "perasc");
const scale = check ? 0.01 : 1;

rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

/** @typedef {Record<string, (...args: any[]) => unknown>} Loaded */

/**
 * Builds a Peras source as a managed module for production, and loads it;
 * `option` names its folder.
 * @param {string} source
 * @param {string} option
 */
async function managed(source, option) {
  const name =
    source
      .split("/")
      .pop()
      ?.replace(/\.(peras|apeiron)$/, "") ?? "m";
  const dir = join(work, `${name}-${option}`);
  mkdirSync(dir, { recursive: true });
  const file = join(dir, `${name}.apeiron`);
  writeFileSync(file, readFileSync(source));
  const r = spawnSync(
    perasc,
    ["build", file, "--out", dir, "--allocator", "tlsf", "--color", "never"],
    { encoding: "utf8", env: process.env },
  );
  if (r.status !== 0) throw new Error(`${source}, ${option}: ${r.stderr}`);
  const js = readFileSync(join(dir, `${name}.js`));
  /** @type {unknown} */
  const m = await import(join(dir, `${name}.js`));
  return { m: /** @type {Loaded} */ (m), bytes: { raw: js.length, gzip: gzipSync(js).length } };
}

/**
 * A TypeScript file, transpiled and loaded.
 * @param {string} source
 */
async function typescript(source) {
  const name = source.split("/").pop()?.replace(".ts", "") ?? "t";
  const file = join(work, `${name}.ts.js`);
  writeFileSync(
    file,
    ts.transpileModule(readFileSync(source, "utf8"), {
      compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
    }).outputText,
  );
  /** @type {unknown} */
  const m = await import(file);
  return /** @type {Loaded} */ (m);
}

/** The kernels of bench/managed/kernels.apeiron, written by hand as scripts/bench-managed.mjs writes them. @type {Loaded} */
const hand = {
  sum(/** @type {number[]} */ xs) {
    let s = 0;
    // eslint-disable-next-line @typescript-eslint/prefer-for-of
    for (let i = 0; i < xs.length; i++) s = (s + (xs[i] ?? 0)) | 0;
    return s;
  },
  step(/** @type {number[]} */ xs, /** @type {number[]} */ vs) {
    for (let i = 0; i < xs.length; i++) {
      let v = vs[i] ?? 0;
      const x = (xs[i] ?? 0) + v * 0.01;
      if (x < 0 || x > 10) v = -v;
      vs[i] = v;
      xs[i] = Math.sqrt(x) * 0.5 + x * 0.5;
    }
  },
  words(/** @type {string} */ t) {
    /** @type {Map<string, number>} */
    const counts = new Map();
    for (const w of t.split(" ")) counts.set(w, (counts.get(w) ?? 0) + 1);
    return counts.size;
  },
  gravity(/** @type {number} */ n) {
    const bodies = [];
    for (let i = 0; i < n; i++)
      bodies.push({ x: Math.sin(i * 0.37) * 10, y: Math.cos(i * 0.71) * 10, mass: 1 + (i % 3) });
    let energy = 0;
    for (let i = 0; i < bodies.length; i++) {
      for (let j = i + 1; j < bodies.length; j++) {
        const a = bodies[i];
        const b = bodies[j];
        if (a === undefined || b === undefined) continue;
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        energy -= (a.mass * b.mass) / Math.sqrt(dx * dx + dy * dy + 0.01);
      }
    }
    return energy;
  },
};

/** @param {ArrayBufferView} v */
const sha = (v) =>
  createHash("sha256")
    .update(Buffer.from(v.buffer, v.byteOffset, v.byteLength))
    .digest("hex");
/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? Number.NaN;

/**
 * A workload on one module: its timed call, and a digest of what it computed.
 * @typedef {{ run: () => unknown, digest: () => string }} Run
 * @typedef {{ label: string, source: string, reference: () => Promise<Loaded>, use: (m: Loaded) => Run }} Workload
 */
const [pw, ph] = check ? [256, 192] : [1024, 768];
const gs = check ? 128 : 1024;
const n = Math.round(1_000_000 * scale);
const ints = Array.from({ length: n }, (_, i) => Math.imul(i, -1640531535));
const xs0 = Array.from({ length: n }, (_, i) => (i % 1000) * 0.01);
const vs0 = xs0.map((x) => Math.sin(x * 7));
const text = Array.from({ length: n / 2 }, (_, i) => `w${String((i * 7919) % 5000)}`).join(" ");
const bodies = Math.round(2000 * Math.sqrt(scale));
const kernels = join(root, "bench", "managed", "kernels.apeiron");
/** @type {Workload[]} */
const workloads = [
  {
    label: `W1, the pond, frame 600 at ${String(pw)} × ${String(ph)} with 1,000 lanterns, against pond.ts`,
    source: join(root, "bench", "lanterns", "pond.peras"),
    reference: () => typescript(join(root, "bench", "lanterns", "pond.ts")),
    use: (m) => {
      const pixels = new Int32Array(pw * ph);
      const depth = new Float64Array(pw * ph);
      const light = new Float64Array(3 * pw * ph);
      let filled = 0;
      return {
        run: () => (filled = Number(m.render?.(pixels, depth, light, pw, ph, 1000, 600))),
        digest: () => `${String(filled)} ${sha(pixels)}`,
      };
    },
  },
  {
    label: `W2, the garden, one frame at ${String(gs)} × ${String(gs)}, against garden.ts`,
    source: join(root, "bench", "garden", "garden.peras"),
    reference: () => typescript(join(root, "bench", "garden", "garden.ts")),
    use: (m) => {
      const pixels = new Uint32Array(gs * gs);
      return {
        run: () => m.render?.(pixels, gs, gs, 1, 0.5, 0.5),
        digest: () => sha(pixels),
      };
    },
  },
  {
    label: `W3, sum of ${String(n)} i32, wrapping, against JavaScript`,
    source: kernels,
    reference: () => Promise.resolve(hand),
    use: (m) => {
      let result = 0;
      return { run: () => (result = Number(m.sum?.(ints))), digest: () => String(result) };
    },
  },
  {
    label: `W3, ${String(n)} particles, one step, inout arrays, against JavaScript`,
    source: kernels,
    reference: () => Promise.resolve(hand),
    use: (m) => {
      let out = "";
      return {
        run: () => {
          const [a, b] = [xs0.slice(), vs0.slice()];
          m.step?.(a, b);
          out = `${String(a[n - 1])} ${String(b[n >> 1])}`;
        },
        digest: () => out,
      };
    },
  },
  {
    label: `W3, count ${String(n / 2)} words, Map<String, i32>, against JavaScript`,
    source: kernels,
    reference: () => Promise.resolve(hand),
    use: (m) => {
      let result = 0;
      return { run: () => (result = Number(m.words?.(text))), digest: () => String(result) };
    },
  },
  {
    label: `W3, energy of ${String(bodies)} bodies, structures, against JavaScript`,
    source: kernels,
    reference: () => Promise.resolve(hand),
    use: (m) => {
      let result = 0;
      return { run: () => (result = Number(m.gravity?.(bodies))), digest: () => String(result) };
    },
  },
];

const lines = [
  `Machine: ${cpus()[0]?.model ?? "unknown"}, ${String(cpus().length)} cores; Node ${process.version}; ${String(ROUNDS)} rounds; options ${OPTIONS.join(", ")}, each a production build.`,
  "",
];
/** @type {Map<string, Promise<{ m: Loaded, bytes: { raw: number, gzip: number } }>>} */
const built = new Map();
for (const { label, source, reference, use } of workloads) {
  /** @type {{ name: string, bytes: { raw: number, gzip: number } | undefined, run: Run }[]} */
  const sides = [{ name: "reference", bytes: undefined, run: use(await reference()) }];
  for (const option of OPTIONS) {
    const key = `${source} ${option}`;
    if (!built.has(key)) built.set(key, managed(source, option));
    const b = await /** @type {Promise<{ m: Loaded, bytes: { raw: number, gzip: number } }>} */ (
      built.get(key)
    );
    sides.push({ name: option, bytes: b.bytes, run: use(b.m) });
  }
  for (const side of sides) side.run.run();
  const want = sides[0]?.run.digest();
  for (const side of sides) {
    const got = side.run.digest();
    if (got !== want) throw new Error(`${label}, ${side.name}: ${got}, not ${String(want)}`);
  }
  const times = sides.map(() => /** @type {number[]} */ ([]));
  const cpu = sides.map(() => 0);
  const wall = sides.map(() => 0);
  for (let round = 0; round < ROUNDS; round++) {
    const order = sides.map((_, i) => i);
    if (round % 2 === 1) order.reverse();
    for (const i of order) {
      const c0 = process.cpuUsage();
      const t0 = performance.now();
      sides[i]?.run.run();
      const t = performance.now() - t0;
      const c = process.cpuUsage(c0);
      times[i]?.push(t);
      wall[i] = (wall[i] ?? 0) + t;
      cpu[i] = (cpu[i] ?? 0) + (c.user + c.system) / 1000;
    }
  }
  lines.push(`### ${label}`, "");
  lines.push("| Version | Median | / reference, median (range) | CPU / wall | Bytes (gzip) |");
  lines.push("| --- | ---: | --- | ---: | ---: |");
  sides.forEach(({ name, bytes }, i) => {
    const own = times[i] ?? [];
    const ratios = own.map((t, k) => t / (times[0]?.[k] ?? Number.NaN));
    const size = bytes === undefined ? "" : `${String(bytes.raw)} (${String(bytes.gzip)})`;
    lines.push(
      `| ${name} | ${median(own).toFixed(2)} ms | ${median(ratios).toFixed(3)} (${Math.min(...ratios).toFixed(3)} to ${Math.max(...ratios).toFixed(3)}) | ${((cpu[i] ?? 0) / (wall[i] ?? 1)).toFixed(2)} | ${size} |`,
    );
  });
  lines.push("");
}

console.log(lines.join("\n"));
if (!check) {
  mkdirSync(join(root, "bench", "index"), { recursive: true });
  writeFileSync(
    join(root, "bench", "index", "CURRENT.md"),
    `# How a managed module checks its indices: the build of today\n\nGenerated by \`node scripts/bench-index.mjs\`, for [decision 0037](../../docs/en/decisions/0037-index-checks-in-managed-modules.md).\n\n${lines.join("\n")}`,
  );
}
