// Web Workers benchmark: one worker, with its own instance of the Peras
// module. It takes a part of the data, copies it into the module's memory,
// runs the kernel, copies the result back and sends it, transferred. Its
// arrays are kept between calls, made again when a part has another size.
//
// The ceiling of shared memory: `load` keeps a part in the module's memory,
// `reset` puts it back as it was, `go` runs the kernel on it and answers
// with nothing, `fetch` sends the result to check it. Only `go` is timed:
// the data never crosses, as with memory every worker shares.
import * as peras from "./kernels.js";

/** @typedef {import("./workers.js").Task} Task */

/** @typedef {import("./kernels.js").PerasArray<"f64">} F64s */
/** @typedef {import("./kernels.js").PerasArray<"i32">} I32s */
/** @type {Map<string, F64s>} */
const f64s = new Map();
/** @type {Map<string, I32s>} */
const i32s = new Map();

/**
 * The array of `f64` kept under `name`, of `n` elements.
 * @param {string} name
 * @param {number} n
 * @returns {F64s}
 */
function f64(name, n) {
  const old = f64s.get(name);
  if (old?.length === n) return old;
  old?.release();
  const made = peras.__perasHeap.array("f64", n);
  f64s.set(name, made);
  return made;
}

/**
 * The array of `i32` kept under `name`, of `n` elements.
 * @param {string} name
 * @param {number} n
 * @returns {I32s}
 */
function i32(name, n) {
  const old = i32s.get(name);
  if (old?.length === n) return old;
  old?.release();
  const made = peras.__perasHeap.array("i32", n);
  i32s.set(name, made);
  return made;
}

/**
 * The worker's own scope: the DOM's types, which this file is checked with,
 * do not describe it.
 * @type {{
 *   onmessage: ((event: MessageEvent<Task>) => void) | null,
 *   postMessage: (message: unknown, options?: { transfer?: Transferable[] }) => void,
 * }}
 */
const scope = /** @type {never} */ (globalThis);

/** The part `load` keeps, as it came, for `reset`. */
/** @type {{ xs: Float64Array, vs: Float64Array, pixels: Int32Array, steps: number }} */
const kept = {
  xs: new Float64Array(0),
  vs: new Float64Array(0),
  pixels: new Int32Array(0),
  steps: 0,
};
let checksum = 0;

/**
 * The ceiling's messages; false for another.
 * @param {Task} data
 */
function resident(data) {
  if (data.kind === "load") {
    kept.xs = data.xs;
    kept.vs = data.vs;
    kept.pixels = data.pixels;
    kept.steps = data.steps;
    f64("rxs", data.xs.length);
    f64("rvs", data.vs.length);
    i32("rpixels", data.pixels.length);
  } else if (data.kind === "reset") {
    f64("rxs", kept.xs.length).view().set(kept.xs);
    f64("rvs", kept.vs.length).view().set(kept.vs);
    i32("rpixels", kept.pixels.length).view().set(kept.pixels);
  } else if (data.kind === "go") {
    if (data.kernel === "grayscale") checksum = peras.grayscale(i32("rpixels", kept.pixels.length));
    else peras.particles(f64("rxs", kept.xs.length), f64("rvs", kept.vs.length), kept.steps);
  } else if (data.kind === "fetch") {
    scope.postMessage({
      kind: "fetched",
      xs: Float64Array.from(f64("rxs", kept.xs.length).view()),
      pixels: Int32Array.from(i32("rpixels", kept.pixels.length).view()),
      checksum,
    });
    return true;
  } else {
    return false;
  }
  scope.postMessage({ kind: "done" });
  return true;
}

scope.onmessage = ({ data }) => {
  if (resident(data)) return;
  if (data.kind === "particles") {
    const xs = f64("xs", data.xs.length);
    const vs = f64("vs", data.vs.length);
    xs.view().set(data.xs);
    vs.view().set(data.vs);
    peras.particles(xs, vs, data.steps);
    data.xs.set(xs.view());
    data.vs.set(vs.view());
    scope.postMessage(data, { transfer: [data.xs.buffer, data.vs.buffer] });
  } else if (data.kind === "grayscale") {
    const pixels = i32("pixels", data.pixels.length);
    pixels.view().set(data.pixels);
    const checksum = peras.grayscale(pixels);
    data.pixels.set(pixels.view());
    scope.postMessage({ ...data, checksum }, { transfer: [data.pixels.buffer] });
  } else if (data.kind === "words") {
    scope.postMessage({ ...data, count: peras.words(data.text) });
  }
};
scope.postMessage("ready");
