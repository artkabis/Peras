// Backend benchmark: compiles bench/backends/kernels.peras with both code generators
// (direct WebAssembly, and Rust through cargo and wasm-bindgen), checks that every
// build computes the same results as a JavaScript reference, then measures build
// time, module size and run time. Writes bench/backends/RESULTS.md.
// Usage: node scripts/bench-backends.mjs   (needs pnpm install, rustup, wasm-bindgen 0.2.130)
//        node scripts/bench-backends.mjs --check   only builds and compares results (CI):
//        fails as soon as the backends disagree, measures nothing, writes nothing.
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { cpus } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { gzipSync } from "node:zlib";

const check = process.argv.includes("--check");
const root = join(import.meta.dirname, "..");
const work = join(root, "target", "bench-backends");
const source = readFileSync(join(root, "bench", "backends", "kernels.peras"), "utf8");
const perasc = join(root, "target", "release", "perasc");
const wasmOpt = join(root, "node_modules", ".bin", "wasm-opt");
const features = [
  "--enable-sign-ext",
  "--enable-nontrapping-float-to-int",
  "--enable-bulk-memory",
  "--enable-mutable-globals",
  "--enable-reference-types",
  "--enable-multivalue",
];

/**
 * Runs a command, failing loudly, and returns its standard output.
 * @param {string} command
 * @param {string[]} args
 * @returns {string}
 */
function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`${command} ${args.join(" ")}\n${result.stdout}${result.stderr}`);
  return result.stdout;
}

/**
 * Median of a list of numbers.
 * @param {number[]} values
 * @returns {number}
 */
function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

/**
 * Wall-clock milliseconds of a call.
 * @param {() => void} task
 * @returns {number}
 */
function timed(task) {
  const start = performance.now();
  task();
  return performance.now() - start;
}

/**
 * The kernels with one more function whose body changes: a real edit for the build.
 * @param {number} k
 * @returns {string}
 */
function variant(k) {
  return `${source}\nexport fn edit(): i32 {\n  return ${String(k)};\n}\n`;
}

/**
 * Builds a variant of the kernels with one backend and returns the build time.
 * @param {"direct" | "rust"} backend
 * @param {number} k
 * @returns {number}
 */
function build(backend, k) {
  const file = join(work, "src", "kernels.peras");
  writeFileSync(file, k < 0 ? source : variant(k));
  return timed(() =>
    run(perasc, [
      "build",
      file,
      "--backend",
      backend,
      "--out",
      join(work, backend),
      "--lang",
      "en",
    ]),
  );
}

// 1. Build the compiler itself, optimized, so that its own speed is measured fairly.
run("cargo", ["build", "--quiet", "--release", "-p", "perasc"]);
rmSync(work, { recursive: true, force: true });
mkdirSync(join(work, "src"), { recursive: true });

// 2. Build times. The Rust backend's first build compiles wasm-bindgen; later
// builds after an edit reuse cargo's cache. The direct backend has no cache.
const directTimes = check ? [] : [1, 2, 3, 4, 5].map((k) => build("direct", k));
const rustCold = check ? 0 : build("rust", 0);
const rustWarm = check ? [] : [1, 2, 3].map((k) => build("rust", k));

// 3. Final builds of the unedited kernels, then wasm-opt -O3 on both.
build("direct", -1);
build("rust", -1);
// The same direct build without the IR optimizations, to measure what they bring.
const plainFile = join(work, "src", "kernels.peras");
run(perasc, [
  "build",
  plainFile,
  "--backend",
  "direct",
  "--no-optimize",
  "--out",
  join(work, "direct-plain"),
  "--lang",
  "en",
]);
const modules = {
  direct: join(work, "direct", "kernels.wasm"),
  rust: join(work, "rust", "peras_kernels_bg.wasm"),
};
mkdirSync(join(work, "opt"), { recursive: true });
cpSync(join(work, "rust"), join(work, "rust-opt"), { recursive: true });
const optimized = {
  direct: join(work, "opt", "direct.wasm"),
  rust: join(work, "rust-opt", "peras_kernels_bg.wasm"),
};
run(wasmOpt, ["-O3", ...features, modules.direct, "-o", optimized.direct]);
run(wasmOpt, ["-O3", ...features, modules.rust, "-o", optimized.rust]);

/**
 * Raw and gzip sizes of a file.
 * @param {string} path
 * @returns {{ raw: number, gzip: number }}
 */
function size(path) {
  const bytes = readFileSync(path);
  return { raw: bytes.length, gzip: gzipSync(bytes, { level: 9 }).length };
}

// 4. Load every build, and a JavaScript reference written the same way.
/** @typedef {{ fib(n: number): number, primes(limit: number): number, mandel(size: number): number, checksum(a: bigint, b: bigint): bigint }} Kernels */

/**
 * Instantiates a module from the direct backend, which needs no imports.
 * @param {string} path
 * @returns {Promise<Kernels>}
 */
async function direct(path) {
  // The WebAssembly global, typed for what this script uses.
  /** @typedef {{ instantiate(bytes: Uint8Array): Promise<{ instance: { exports: unknown } }> }} WasmApi */
  const global = /** @type {unknown} */ (Reflect.get(globalThis, "WebAssembly"));
  const { instance } = await /** @type {WasmApi} */ (global).instantiate(readFileSync(path));
  return /** @type {Kernels} */ (instance.exports);
}

const require = createRequire(import.meta.url);
// wasm-bindgen's Node target writes CommonJS, and the repository is an ES module package.
for (const dir of ["rust", "rust-opt"]) {
  writeFileSync(join(work, dir, "package.json"), '{ "type": "commonjs" }\n');
}
/**
 * Loads a module generated by wasm-bindgen for Node.
 * @param {string} dir
 * @returns {Kernels}
 */
function bindgen(dir) {
  const loaded = /** @type {unknown} */ (require(join(dir, "peras_kernels.js")));
  return /** @type {Kernels} */ (loaded);
}

/** @type {Kernels} */
const reference = {
  fib(n) {
    return n < 2 ? n : (this.fib(n - 1) + this.fib(n - 2)) | 0;
  },
  primes(limit) {
    let count = 0;
    for (let n = 2; n < limit; n++) {
      let prime = true;
      for (let d = 2; Math.imul(d, d) <= n; d++) {
        if (n % d === 0) {
          prime = false;
          break;
        }
      }
      if (prime) count++;
    }
    return count;
  },
  mandel(size) {
    let inside = 0;
    const scale = 3.0 / size;
    for (let py = 0; py < size; py++) {
      for (let px = 0; px < size; px++) {
        const cx = px * scale - 2.0;
        const cy = py * scale - 1.5;
        let x = 0.0;
        let y = 0.0;
        let step = 0;
        while (step < 64 && x * x + y * y <= 4.0) {
          const next = x * x - y * y + cx;
          y = 2.0 * x * y + cy;
          x = next;
          step++;
        }
        if (step === 64) inside++;
      }
    }
    return inside;
  },
  // JavaScript has no 64-bit integers: BigInt, wrapped to 64 bits, is the only way.
  checksum(a, b) {
    let h = 1469598103934665603n;
    for (let i = 0n; i < a; i++) {
      h = BigInt.asIntN(64, (h ^ BigInt.asIntN(64, i * b)) * 1099511628211n);
      if (h < 0n) h = BigInt.asIntN(64, -h);
    }
    return h % 1000000007n;
  },
};

/** @type {[string, Kernels][]} */
const builds = [
  ["direct", await direct(modules.direct)],
  ["direct, IR not optimized", await direct(join(work, "direct-plain", "kernels.wasm"))],
  ["direct + wasm-opt", await direct(optimized.direct)],
  ["rust", bindgen(join(work, "rust"))],
  ["rust + wasm-opt", bindgen(join(work, "rust-opt"))],
  ["JavaScript", reference],
];

// 5. Differential check: every build must agree with the reference on every input.
/** @type {[string, (k: Kernels) => unknown][]} */
const cases = [
  ...[0, 1, 2, 10, 20, 25].map(
    (n) =>
      /** @type {[string, (k: Kernels) => unknown]} */ ([`fib(${String(n)})`, (k) => k.fib(n)]),
  ),
  ...[2, 3, 100, 1000, 10000].map(
    (n) =>
      /** @type {[string, (k: Kernels) => unknown]} */ ([
        `primes(${String(n)})`,
        (k) => k.primes(n),
      ]),
  ),
  ...[1, 10, 40, 100].map(
    (n) =>
      /** @type {[string, (k: Kernels) => unknown]} */ ([
        `mandel(${String(n)})`,
        (k) => k.mandel(n),
      ]),
  ),
  .../** @type {[bigint, bigint][]} */ ([
    [0n, 1n],
    [1n, 7n],
    [1000n, 7n],
    [100000n, -3n],
  ]).map(
    ([a, b]) =>
      /** @type {[string, (k: Kernels) => unknown]} */ ([
        `checksum(${String(a)}, ${String(b)})`,
        (k) => k.checksum(a, b),
      ]),
  ),
];
let agreements = 0;
for (const [label, call] of cases) {
  const expected = call(reference);
  for (const [name, kernels] of builds) {
    const actual = call(kernels);
    if (actual !== expected)
      throw new Error(`${name}: ${label} = ${String(actual)}, expected ${String(expected)}`);
    agreements++;
  }
}

if (check) {
  console.log(
    `Backends agree: ${String(builds.length - 1)} builds matched the JavaScript reference on ${String(cases.length)} inputs (${String(agreements)} comparisons).`,
  );
} else {
  // 6. Run times: median of 15 runs after 3 warm-up runs.
  /** @type {[string, (k: Kernels) => unknown][]} */
  const workloads = [
    ["fib(32)", (k) => k.fib(32)],
    ["primes(1 000 000)", (k) => k.primes(1_000_000)],
    ["mandel(600)", (k) => k.mandel(600)],
    ["checksum(5 000 000, 7)", (k) => k.checksum(5_000_000n, 7n)],
  ];
  /** @type {Map<string, number[]>} */
  const times = new Map();
  for (const [, kernels] of builds) {
    /** @type {number[]} */
    const row = [];
    for (const [, call] of workloads) {
      call(kernels);
      call(kernels);
      row.push(median(Array.from({ length: 15 }, () => timed(() => call(kernels)))));
    }
    times.set(builds.find(([, k]) => k === kernels)?.[0] ?? "", row);
  }

  // 7. Report.
  const ms = (/** @type {number} */ v) =>
    v < 10 ? v.toFixed(1) : Math.round(v).toLocaleString("en-US");
  const kb = (/** @type {number} */ v) => v.toLocaleString("en-US");
  const sizes = {
    direct: size(modules.direct),
    plain: size(join(work, "direct-plain", "kernels.wasm")),
    directOpt: size(optimized.direct),
    rust: size(modules.rust),
    rustOpt: size(optimized.rust),
  };
  const lines = [
    "# Backend benchmark — results",
    "",
    "Generated by `node scripts/bench-backends.mjs` from `bench/backends/kernels.peras`.",
    "",
    `Machine: ${cpus()[0]?.model ?? "unknown CPU"}, ${String(cpus().length)} cores. ${run("rustc", ["--version"]).trim()}, Node ${process.version}, wasm-bindgen 0.2.130, Binaryen ${run(wasmOpt, ["--version"]).trim()}.`,
    "",
    "## Build time (ms, wall clock, perasc release build)",
    "",
    "| Backend | First build | Rebuild after an edit (median) | Tools needed |",
    "| --- | --- | --- | --- |",
    `| direct | ${ms(directTimes[0] ?? 0)} | ${ms(median(directTimes.slice(1)))} | perasc |`,
    `| rust | ${ms(rustCold)} | ${ms(median(rustWarm))} | perasc, rustup, cargo, wasm-bindgen |`,
    "",
    "## Module size (bytes)",
    "",
    "| Backend | Raw | Raw, gzip | After wasm-opt -O3 | wasm-opt, gzip |",
    "| --- | --- | --- | --- | --- |",
    `| direct | ${kb(sizes.direct.raw)} | ${kb(sizes.direct.gzip)} | ${kb(sizes.directOpt.raw)} | ${kb(sizes.directOpt.gzip)} |`,
    `| direct, IR not optimized | ${kb(sizes.plain.raw)} | ${kb(sizes.plain.gzip)} | — | — |`,
    `| rust | ${kb(sizes.rust.raw)} | ${kb(sizes.rust.gzip)} | ${kb(sizes.rustOpt.raw)} | ${kb(sizes.rustOpt.gzip)} |`,
    "",
    "The Rust sizes leave out the JavaScript glue that wasm-bindgen writes next to the module.",
    "",
    "## Run time in Node (ms, median of 15 after 3 warm-up runs)",
    "",
    `| Build | ${workloads.map(([label]) => label).join(" | ")} |`,
    `| --- | ${workloads.map(() => "---").join(" | ")} |`,
    ...[...times].map(([name, row]) => `| ${name} | ${row.map(ms).join(" | ")} |`),
    "",
    `Every build gave the same result as the JavaScript reference on ${String(cases.length)} inputs (${String(agreements)} comparisons).`,
    "The JavaScript `checksum` must use BigInt, since JavaScript has no 64-bit integers.",
    "",
  ];
  writeFileSync(join(root, "bench", "backends", "RESULTS.md"), lines.join("\n"));
  console.log(lines.join("\n"));
}
