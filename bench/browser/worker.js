// Web Workers benchmark: one worker, with its own instance of the Peras
// module. It takes a part of the data, copies it into the module's memory,
// runs the kernel, copies the result back and sends it, transferred. Its
// arrays are kept between calls, made again when a part has another size.
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

scope.onmessage = ({ data }) => {
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
  } else {
    scope.postMessage({ ...data, count: peras.words(data.text) });
  }
};
scope.postMessage("ready");
