// Browser benchmark: runs in the page. Makes the inputs, checks that Peras and
// JavaScript give the same result, then times them, alternating, in the same
// page. `run` compares one build of Peras with JavaScript, for
// scripts/bench-browser.mjs; `ab` compares two builds, for
// scripts/bench-ab.mjs.
import * as js from "./reference.js";

/** @typedef {typeof import("./kernels.js")} Peras */
/**
 * @typedef {object} Build One build of the kernels, and the name its
 * variants carry: empty for the benchmark, "A" or "B" for a comparison.
 * @property {string} name
 * @property {Peras} peras
 * @property {{ allocations(): number } | undefined} [runtime] Its runtime, to count allocations.
 */
/**
 * @typedef {object} Kernel A kernel, ready to run.
 * @property {string} name
 * @property {string} what
 * @property {Variant[]} variants JavaScript's, then each build's.
 * @property {() => void} check Throws unless every variant gave the same result.
 * @property {() => void} release Gives back the arrays it made in the modules.
 */

/**
 * @typedef {object} Variant One way to run a kernel.
 * @property {string} label
 * @property {() => unknown} call
 * @property {(() => unknown) | undefined} [prepare] Runs before each call, outside the time.
 * @property {unknown} [result]
 */
/** @typedef {{ label: string, cold: number, median: number, min: number }} Timing */
/** @typedef {{ name: string, what: string, result: Timing[] }} Case */
/** @typedef {{ build: string, label: string, allocations: number }} Count */
/** @typedef {{ scale: number, runs: number, warm: number }} Config */

/**
 * A deterministic stream of numbers in [0, 1): the same inputs every run.
 * @param {number} seed
 */
function random(seed) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;

/**
 * Times each variant: one cold call first, then `warm` calls not counted,
 * then `runs` calls, the variants alternating so that each sees the same
 * state of the page. `prepare` runs before each call, outside the time.
 * @param {Variant[]} variants
 * @param {number} runs
 * @param {number} warm
 * @returns {Timing[]}
 */
function time(variants, runs, warm) {
  const out = variants.map((v) => ({ v, cold: 0, times: /** @type {number[]} */ ([]) }));
  for (let r = -1; r < warm + runs; r++) {
    for (const o of out) {
      o.v.prepare?.();
      const t0 = performance.now();
      o.v.result = o.v.call();
      const t = performance.now() - t0;
      if (r === -1) o.cold = t;
      else if (r >= warm) o.times.push(t);
    }
  }
  return out.map((o) => ({
    label: o.v.label,
    cold: o.cold,
    median: median(o.times),
    min: Math.min(...o.times),
  }));
}

/**
 * Throws unless every variant gave the same result.
 * @param {string} name
 * @param {unknown[]} results
 */
function same(name, results) {
  const [first, ...rest] = results;
  for (const r of rest) {
    if (!Object.is(r, first)) {
      const shown = results.map((x) =>
        typeof x === "number" || typeof x === "bigint" ? x.toString() : JSON.stringify(x),
      );
      throw new Error(`${name}: results differ: ${shown.join(" / ")}`);
    }
  }
}

/**
 * Text of words separated by one or two spaces, accented ones included.
 * @param {number} bytes
 */
function text(bytes) {
  const vocabulary = [
    "the",
    "web",
    "été",
    "naïve",
    "compile",
    "rust",
    "wasm",
    "données",
    "a",
    "loop",
  ];
  const next = random(7);
  const parts = [];
  let size = 0;
  while (size < bytes) {
    const w = vocabulary[Math.floor(next() * vocabulary.length)] ?? "";
    parts.push(w, next() < 0.1 ? "  " : " ");
    size += w.length + 1;
  }
  return parts.join("");
}

/**
 * The variant of a build: its label carries the build's name.
 * @param {Build} build
 * @param {Variant} variant
 * @returns {Variant}
 */
const of = (build, variant) =>
  build.name === "" ? variant : { ...variant, label: `${build.name}: ${variant.label}` };

/**
 * Every kernel, with JavaScript's variants and those of each build, its
 * inputs made once.
 * @param {Build[]} builds
 * @param {number} scale
 * @returns {Kernel[]}
 */
export function kernels(builds, scale) {
  /** @param {number} x */
  const n = (x) => Math.max(1, Math.round(x * scale));
  /** @param {number} x */
  const fmt = (x) => x.toLocaleString("en");
  /** @param {Variant[]} variants */
  const results = (variants) => variants.map((v) => v.result);
  /** @type {Kernel[]} */
  const out = [];

  // Particles: the same arrays, zero copy with a PerasArray, or copied in
  // and back out when the function gets plain typed arrays.
  {
    const count = n(1_000_000);
    const next = random(1);
    const x0 = Float64Array.from({ length: count }, () => next());
    const v0 = Float64Array.from({ length: count }, () => next() * 2 - 1);
    const jx = new Float64Array(count);
    const jv = new Float64Array(count);
    const steps = 10;
    const arrays = builds.map((b) => ({
      px: b.peras.__perasHeap.array("f64", count),
      pv: b.peras.__perasHeap.array("f64", count),
      cx: new Float64Array(count),
      cv: new Float64Array(count),
    }));
    /** @type {Variant[]} */
    const variants = [
      {
        label: "JavaScript",
        prepare: () => {
          jx.set(x0);
          jv.set(v0);
        },
        call: () => js.particles(jx, jv, steps),
      },
    ];
    builds.forEach((b, k) => {
      const a = arrays[k];
      variants.push(
        of(b, {
          label: "Peras, zero copy",
          prepare: () => {
            a.px.view().set(x0);
            a.pv.view().set(v0);
          },
          call: () => b.peras.particles(a.px, a.pv, steps),
        }),
        of(b, {
          label: "Peras, copied",
          prepare: () => {
            a.cx.set(x0);
            a.cv.set(v0);
          },
          call: () => b.peras.particles(a.cx, a.cv, steps),
        }),
      );
    });
    out.push({
      name: "particles",
      what: `${fmt(count)} particles, ${String(steps)} steps`,
      variants,
      check: () => {
        same("particles", results(variants));
        same("particles, positions", [
          jx[count - 1],
          ...arrays.flatMap((a) => [a.px.view()[count - 1], a.cx[count - 1]]),
        ]);
      },
      release: () => {
        for (const a of arrays) {
          a.px.release();
          a.pv.release();
        }
      },
    });
  }

  // Pixels: a 2048 × 2048 image, packed RGBA, to grey in place.
  {
    const count = n(2048 * 2048);
    const next = random(2);
    const image = Int32Array.from({ length: count }, () => Math.floor(next() * 4294967296) | 0);
    const jp = new Int32Array(count);
    const arrays = builds.map((b) => b.peras.__perasHeap.array("i32", count));
    /** @type {Variant[]} */
    const variants = [
      {
        label: "JavaScript",
        prepare: () => {
          jp.set(image);
        },
        call: () => js.grayscale(jp),
      },
    ];
    builds.forEach((b, k) => {
      const pp = arrays[k];
      variants.push(
        of(b, {
          label: "Peras, zero copy",
          prepare: () => {
            pp.view().set(image);
          },
          call: () => b.peras.grayscale(pp),
        }),
      );
    });
    out.push({
      name: "grayscale",
      what: `${fmt(count)} pixels`,
      variants,
      check: () => {
        same("grayscale", results(variants));
        same("grayscale, pixels", [jp[count >> 1], ...arrays.map((pp) => pp.view()[count >> 1])]);
      },
      release: () => {
        for (const pp of arrays) pp.release();
      },
    });
  }

  // Bodies: structures in an array, every pair at every step.
  {
    const count = n(1000);
    const steps = 5;
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => js.nbody(count, steps) },
      ...builds.map((b) => of(b, { label: "Peras", call: () => b.peras.nbody(count, steps) })),
    ];
    out.push({
      name: "nbody",
      what: `${String(count)} bodies, ${String(steps)} steps`,
      variants,
      check: () => {
        same("nbody", results(variants));
      },
      release: () => undefined,
    });
  }

  // 64-bit hashing: BigInt in JavaScript, i64 in Peras.
  {
    const count = n(2_000_000);
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => js.hash64(count) },
      ...builds.map((b) => of(b, { label: "Peras", call: () => b.peras.hash64(count) })),
    ];
    out.push({
      name: "hash64",
      what: `FNV-1a over ${fmt(count)} integers`,
      variants,
      check: () => {
        same("hash64", results(variants));
      },
      release: () => undefined,
    });
  }

  // Sorting numbers in place.
  {
    const count = n(1_000_000);
    const next = random(3);
    const data = Float64Array.from({ length: count }, () => next() * 1e6);
    const jt = new Float64Array(count);
    /** @type {number[]} */
    let ja = [];
    const arrays = builds.map((b) => b.peras.__perasHeap.array("f64", count));
    /** @type {Variant[]} */
    const variants = [
      {
        label: "JavaScript, Float64Array",
        prepare: () => {
          jt.set(data);
        },
        call: () => js.sorted(jt),
      },
      {
        label: "JavaScript, number[]",
        prepare: () => {
          ja = Array.from(data);
        },
        call: () => js.sortedArray(ja),
      },
    ];
    builds.forEach((b, k) => {
      const pp = arrays[k];
      variants.push(
        of(b, {
          label: "Peras, zero copy",
          prepare: () => {
            pp.view().set(data);
          },
          call: () => b.peras.sorted(pp),
        }),
      );
    });
    out.push({
      name: "sort",
      what: `${fmt(count)} f64`,
      variants,
      check: () => {
        same("sorted", results(variants));
      },
      release: () => {
        for (const pp of arrays) pp.release();
      },
    });
  }

  // Floating-point arithmetic only.
  {
    const size = n(600);
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => js.mandel(size) },
      ...builds.map((b) => of(b, { label: "Peras", call: () => b.peras.mandel(size) })),
    ];
    out.push({
      name: "mandel",
      what: `${String(size)} × ${String(size)}, 64 steps`,
      variants,
      check: () => {
        same("mandel", results(variants));
      },
      release: () => undefined,
    });
  }

  // Text: the string crosses the boundary as UTF-8 at each call.
  {
    const input = text(n(1_000_000));
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => js.words(input) },
      ...builds.map((b) => of(b, { label: "Peras", call: () => b.peras.words(input) })),
    ];
    out.push({
      name: "words",
      what: `${fmt(input.length)} characters`,
      variants,
      check: () => {
        same("words", results(variants));
      },
      release: () => undefined,
    });
  }

  // The cost of a call from JavaScript.
  {
    const calls = n(1_000_000);
    /** @param {(a: number, b: number) => number} f */
    const loop = (f) => {
      let s = 0;
      for (let i = 0; i < calls; i++) s = f(s & 1023, i & 7);
      return s;
    };
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => loop(js.add) },
      ...builds.map((b) => of(b, { label: "Peras", call: () => loop(b.peras.add) })),
    ];
    out.push({
      name: "calls",
      what: `${fmt(calls)} calls of add(a, b)`,
      variants,
      check: () => {
        same("add", results(variants));
      },
      release: () => undefined,
    });
  }
  return out;
}

/**
 * Throws unless every build gave back every block.
 * @param {Build[]} builds
 */
function noLeak(builds) {
  for (const b of builds) {
    const live = b.peras.__perasHeap.live();
    if (live !== 0) throw new Error(`${b.name || "Peras"}: ${String(live)} blocks left alive`);
  }
}

/**
 * Runs every kernel, JavaScript and Peras.
 * @param {Peras} peras
 * @param {Config} config
 * @returns {Case[]}
 */
export function run(peras, { scale, runs, warm }) {
  const builds = [{ name: "", peras }];
  const cases = kernels(builds, scale).map((k) => {
    const result = time(k.variants, runs, warm);
    k.check();
    k.release();
    return { name: k.name, what: k.what, result };
  });
  noLeak(builds);
  return cases;
}

/**
 * Compares two builds: every kernel, their variants and JavaScript's
 * alternating, then the allocations of one call of each variant of each
 * build, counted by its runtime.
 * @param {Build[]} builds
 * @param {Config} config
 * @returns {{ cases: Case[], counts: Count[] }}
 */
export function ab(builds, { scale, runs, warm }) {
  /** @type {Count[]} */
  const counts = [];
  const cases = kernels(builds, scale).map((k) => {
    const result = time(k.variants, runs, warm);
    k.check();
    for (const b of builds) {
      const runtime = b.runtime;
      if (runtime === undefined) continue;
      for (const v of k.variants.filter((x) => x.label.startsWith(`${b.name}: `))) {
        v.prepare?.();
        const before = runtime.allocations();
        v.call();
        counts.push({
          build: b.name,
          label: `${k.name}, ${v.label.slice(b.name.length + 2)}`,
          allocations: runtime.allocations() - before,
        });
      }
    }
    k.release();
    return { name: k.name, what: k.what, result };
  });
  noLeak(builds);
  return { cases, counts };
}
