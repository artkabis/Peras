// Web Workers benchmark: runs in the page. Splits a kernel's data into N
// parts, sends them to N workers (worker.js), each with its own instance of
// the Peras module, and gathers the results; times that against the kernel
// called directly on the main thread. Every split result is checked equal to
// the direct one, element by element. scripts/bench-workers.mjs drives it.
import * as peras from "./kernels.js";

/**
 * @typedef {{ kind: "particles", xs: Float64Array, vs: Float64Array, steps: number }
 *   | { kind: "grayscale", pixels: Int32Array }
 *   | { kind: "words", text: string }} Task What a worker is sent.
 */
/**
 * @typedef {{ kind: "particles", xs: Float64Array, vs: Float64Array, steps: number }
 *   | { kind: "grayscale", pixels: Int32Array, checksum: number }
 *   | { kind: "words", text: string, count: number }} Answer What a worker sends back.
 */
/** @typedef {{ kernel: string, way: string, median: number }} Row */

/**
 * A deterministic stream of numbers in [0, 1).
 * @param {number} seed
 */
function random(seed) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

/**
 * Bounds of k near-equal slices of `length`.
 * @param {number} length
 * @param {number} k
 */
const bounds = (length, k) => Array.from({ length: k + 1 }, (_, i) => Math.floor((length * i) / k));

/** @param {number[]} xs */
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)] ?? 0;

/**
 * Workers, each with the module loaded.
 * @param {number} k
 * @returns {Promise<Worker[]>}
 */
function pool(k) {
  return Promise.all(
    Array.from({ length: k }, () => {
      /** @type {Promise<Worker>} */
      const ready = new Promise((ok) => {
        const w = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
        w.onmessage = (e) => {
          if (e.data === "ready") ok(w);
        };
      });
      return ready;
    }),
  );
}

/**
 * Sends a task to a worker, its arrays transferred, and waits for its answer.
 * @param {Worker} w
 * @param {Task} task
 * @param {Transferable[]} transfer
 * @returns {Promise<Answer>}
 */
function ask(w, task, transfer) {
  return new Promise((ok) => {
    w.onmessage = (/** @type {MessageEvent<Answer>} */ e) => {
      ok(e.data);
    };
    w.postMessage(task, transfer);
  });
}

/**
 * Times every kernel directly and split across 1, 2 and 4 workers.
 * @param {{ scale: number, runs: number }} config
 * @returns {Promise<{ cores: number, rows: Row[] }>}
 */
export async function measure({ scale, runs }) {
  /** @param {number} x */
  const n = (x) => Math.max(1, Math.round(x * scale));
  const next = random(1);
  const count = n(1_000_000);
  const x0 = Float64Array.from({ length: count }, () => next());
  const v0 = Float64Array.from({ length: count }, () => next() * 2 - 1);
  const pixelCount = n(2048 * 2048);
  const image = Int32Array.from({ length: pixelCount }, () => Math.floor(next() * 4294967296) | 0);
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
  /** @type {string[]} */
  const words = [];
  let size = 0;
  while (size < n(1_000_000)) {
    const w = vocabulary[Math.floor(next() * vocabulary.length)] ?? "";
    words.push(w, next() < 0.1 ? "  " : " ");
    size += w.length + 1;
  }
  const text = words.join("");

  /**
   * The text cut at spaces into k parts, so that no word is split.
   * @param {number} k
   */
  const cuts = (k) => {
    const at = bounds(text.length, k).map((b, i) => {
      if (i === 0 || i === k) return b;
      const space = text.indexOf(" ", b);
      return space < 0 ? text.length : space + 1;
    });
    return at.slice(1).map((end, i) => text.slice(at[i], end));
  };

  // Direct, on the main thread, arrays already in the module's memory.
  const px = peras.__perasHeap.array("f64", count);
  const pv = peras.__perasHeap.array("f64", count);
  const pp = peras.__perasHeap.array("i32", pixelCount);
  /** @param {number} steps */
  const moved = (steps) => () => {
    px.view().set(x0);
    pv.view().set(v0);
    const t0 = performance.now();
    peras.particles(px, pv, steps);
    return performance.now() - t0;
  };
  // 10 steps: little work for the bytes sent; 200: twenty times more.
  /** @type {[string, number][]} */
  const particleKernels = [
    ["particles", 10],
    ["particles, 200 steps", 200],
  ];
  /** @type {Map<string, () => number>} */
  const direct = new Map([
    ...particleKernels.map(
      ([name, steps]) => /** @type {[string, () => number]} */ ([name, moved(steps)]),
    ),
    [
      "grayscale",
      () => {
        pp.view().set(image);
        const t0 = performance.now();
        peras.grayscale(pp);
        return performance.now() - t0;
      },
    ],
    [
      "words",
      () => {
        const t0 = performance.now();
        peras.words(text);
        return performance.now() - t0;
      },
    ],
  ]);
  /** @type {Map<string, Float64Array>} */
  const positions = new Map();
  for (const [name, steps] of particleKernels) {
    moved(steps)();
    positions.set(name, Float64Array.from(px.view()));
  }
  pp.view().set(image);
  const checksum = peras.grayscale(pp);
  const pixels = Int32Array.from(pp.view());
  const wordCount = peras.words(text);

  /**
   * One split run of `kernel` on `workers`, checked; its time in ms.
   * @param {string} kernel
   * @param {Worker[]} workers
   */
  const split = async (kernel, workers) => {
    const k = workers.length;
    const t0 = performance.now();
    const steps = particleKernels.find(([name]) => name === kernel)?.[1];
    if (steps !== undefined) {
      const b = bounds(count, k);
      const back = await Promise.all(
        workers.map((w, i) => {
          const xs = x0.slice(b[i], b[i + 1]);
          const vs = v0.slice(b[i], b[i + 1]);
          return ask(w, { kind: "particles", xs, vs, steps }, [xs.buffer, vs.buffer]);
        }),
      );
      const out = new Float64Array(count);
      back.forEach((r, i) => {
        if (r.kind === "particles") out.set(r.xs, b[i]);
      });
      const ms = performance.now() - t0;
      const want = positions.get(kernel) ?? new Float64Array(0);
      for (let i = 0; i < count; i++) {
        if (!Object.is(out[i], want[i]))
          throw new Error(`${kernel}, ${String(k)} workers, differ at ${String(i)}`);
      }
      return ms;
    }
    if (kernel === "grayscale") {
      const b = bounds(pixelCount, k);
      const back = await Promise.all(
        workers.map((w, i) => {
          const part = image.slice(b[i], b[i + 1]);
          return ask(w, { kind: "grayscale", pixels: part }, [part.buffer]);
        }),
      );
      const out = new Int32Array(pixelCount);
      let xor = 0;
      back.forEach((r, i) => {
        if (r.kind !== "grayscale") return;
        out.set(r.pixels, b[i]);
        xor ^= r.checksum;
      });
      const ms = performance.now() - t0;
      if (xor !== checksum) throw new Error(`grayscale, ${String(k)} workers: checksum`);
      for (let i = 0; i < pixelCount; i++) {
        if (out[i] !== pixels[i])
          throw new Error(`grayscale, ${String(k)} workers, differ at ${String(i)}`);
      }
      return ms;
    }
    const back = await Promise.all(
      cuts(k).map((part, i) => ask(workers[i], { kind: "words", text: part }, [])),
    );
    const total = back.reduce((sum, r) => sum + (r.kind === "words" ? r.count : 0), 0);
    const ms = performance.now() - t0;
    if (total !== wordCount)
      throw new Error(`words, ${String(k)} workers: ${String(total)} against ${String(wordCount)}`);
    return ms;
  };

  const sizes = [1, 2, 4];
  /** @type {Map<number, Worker[]>} */
  const pools = new Map();
  for (const k of sizes) pools.set(k, await pool(k));
  /** @type {Row[]} */
  const rows = [];
  for (const [kernel, run] of direct) {
    /** @type {number[]} */
    const directTimes = [];
    /** @type {Map<number, number[]>} */
    const splitTimes = new Map(sizes.map((k) => [k, []]));
    // Warm-up, then rounds alternating every way.
    for (let r = 0; r < runs + 2; r++) {
      const d = run();
      if (r >= 2) directTimes.push(d);
      for (const k of sizes) {
        const ms = await split(kernel, pools.get(k) ?? []);
        if (r >= 2) splitTimes.get(k)?.push(ms);
      }
    }
    rows.push({ kernel, way: "direct", median: median(directTimes) });
    for (const k of sizes)
      rows.push({ kernel, way: String(k), median: median(splitTimes.get(k) ?? []) });
  }
  for (const ws of pools.values()) for (const w of ws) w.terminate();
  px.release();
  pv.release();
  pp.release();
  return { cores: navigator.hardwareConcurrency, rows };
}
