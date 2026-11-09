// Browser benchmark: runs in the page. Makes the inputs, checks that Peras and
// JavaScript give the same result, then times both, alternating, in the same
// page. Loaded by scripts/bench-browser.mjs.
import * as peras from "./kernels.js";
import * as js from "./reference.js";

/**
 * @typedef {object} Variant One way to run a kernel.
 * @property {string} label
 * @property {() => unknown} call
 * @property {(() => unknown) | undefined} [prepare] Runs before each call, outside the time.
 * @property {unknown} [result]
 */
/** @typedef {{ label: string, cold: number, median: number, min: number }} Timing */
/** @typedef {{ name: string, what: string, result: Timing[] }} Case */
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
 * Runs every kernel both ways.
 * @param {Config} config
 * @returns {Case[]}
 */
export function run({ scale, runs, warm }) {
  /** @type {Case[]} */
  const cases = [];
  /** @param {number} x */
  const n = (x) => Math.max(1, Math.round(x * scale));
  /** @param {number} x */
  const fmt = (x) => x.toLocaleString("en");

  // Particles: the same arrays, zero copy with a PerasArray, or copied in
  // and back out when the function gets plain typed arrays.
  {
    const count = n(1_000_000);
    const next = random(1);
    const x0 = Float64Array.from({ length: count }, () => next());
    const v0 = Float64Array.from({ length: count }, () => next() * 2 - 1);
    const jx = new Float64Array(count);
    const jv = new Float64Array(count);
    const px = peras.__perasHeap.array("f64", count);
    const pv = peras.__perasHeap.array("f64", count);
    const cx = new Float64Array(count);
    const cv = new Float64Array(count);
    const steps = 10;
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
      {
        label: "Peras, zero copy",
        prepare: () => {
          px.view().set(x0);
          pv.view().set(v0);
        },
        call: () => peras.particles(px, pv, steps),
      },
      {
        label: "Peras, copied",
        prepare: () => {
          cx.set(x0);
          cv.set(v0);
        },
        call: () => peras.particles(cx, cv, steps),
      },
    ];
    const result = time(variants, runs, warm);
    same(
      "particles",
      variants.map((v) => v.result),
    );
    same("particles, positions", [jx[count - 1], px.view()[count - 1], cx[count - 1]]);
    px.release();
    pv.release();
    cases.push({
      name: "particles",
      what: `${fmt(count)} particles, ${String(steps)} steps`,
      result,
    });
  }

  // Pixels: a 2048 × 2048 image, packed RGBA, to grey in place.
  {
    const count = n(2048 * 2048);
    const next = random(2);
    const image = Int32Array.from({ length: count }, () => Math.floor(next() * 4294967296) | 0);
    const jp = new Int32Array(count);
    const pp = peras.__perasHeap.array("i32", count);
    /** @type {Variant[]} */
    const variants = [
      {
        label: "JavaScript",
        prepare: () => {
          jp.set(image);
        },
        call: () => js.grayscale(jp),
      },
      {
        label: "Peras, zero copy",
        prepare: () => {
          pp.view().set(image);
        },
        call: () => peras.grayscale(pp),
      },
    ];
    const result = time(variants, runs, warm);
    same(
      "grayscale",
      variants.map((v) => v.result),
    );
    same("grayscale, pixels", [jp[count >> 1], pp.view()[count >> 1]]);
    pp.release();
    cases.push({ name: "grayscale", what: `${fmt(count)} pixels`, result });
  }

  // Bodies: structures in an array, every pair at every step.
  {
    const count = n(1000);
    const steps = 5;
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => js.nbody(count, steps) },
      { label: "Peras", call: () => peras.nbody(count, steps) },
    ];
    const result = time(variants, runs, warm);
    same(
      "nbody",
      variants.map((v) => v.result),
    );
    cases.push({ name: "nbody", what: `${String(count)} bodies, ${String(steps)} steps`, result });
  }

  // 64-bit hashing: BigInt in JavaScript, i64 in Peras.
  {
    const count = n(2_000_000);
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => js.hash64(count) },
      { label: "Peras", call: () => peras.hash64(count) },
    ];
    const result = time(variants, runs, warm);
    same(
      "hash64",
      variants.map((v) => v.result),
    );
    cases.push({
      name: "hash64",
      what: `FNV-1a over ${fmt(count)} integers`,
      result,
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
    const pp = peras.__perasHeap.array("f64", count);
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
      {
        label: "Peras, zero copy",
        prepare: () => {
          pp.view().set(data);
        },
        call: () => peras.sorted(pp),
      },
    ];
    const result = time(variants, runs, warm);
    same(
      "sorted",
      variants.map((v) => v.result),
    );
    pp.release();
    cases.push({ name: "sort", what: `${fmt(count)} f64`, result });
  }

  // Floating-point arithmetic only.
  {
    const size = n(600);
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => js.mandel(size) },
      { label: "Peras", call: () => peras.mandel(size) },
    ];
    const result = time(variants, runs, warm);
    same(
      "mandel",
      variants.map((v) => v.result),
    );
    cases.push({ name: "mandel", what: `${String(size)} × ${String(size)}, 64 steps`, result });
  }

  // Text: the string crosses the boundary as UTF-8 at each call.
  {
    const input = text(n(1_000_000));
    /** @type {Variant[]} */
    const variants = [
      { label: "JavaScript", call: () => js.words(input) },
      { label: "Peras", call: () => peras.words(input) },
    ];
    const result = time(variants, runs, warm);
    same(
      "words",
      variants.map((v) => v.result),
    );
    cases.push({ name: "words", what: `${fmt(input.length)} characters`, result });
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
      { label: "Peras", call: () => loop(peras.add) },
    ];
    const result = time(variants, runs, warm);
    same(
      "add",
      variants.map((v) => v.result),
    );
    cases.push({ name: "calls", what: `${fmt(calls)} calls of add(a, b)`, result });
  }

  if (peras.__perasHeap.live() !== 0)
    throw new Error(`${String(peras.__perasHeap.live())} blocks left alive`);
  return cases;
}
