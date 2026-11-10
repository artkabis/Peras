// Browser benchmark: the other compilers to WebAssembly, behind one shape so
// that runner.js runs them as it runs Peras. The kernels are in rust/ and
// assemblyscript/; scripts/bench-browser.mjs --contenders builds them next to
// this file, into rust/ and as/.
//
// Arrays of numbers stay in each module's memory, as a `PerasArray` does:
// `view()` is a typed array over them, with no copy, valid until the next
// call into the module.

/**
 * @typedef {object} Numbers An array in a contender's memory.
 * @property {() => Float64Array | Int32Array} view
 * @property {unknown} handle What the contender's functions take.
 * @property {() => void} release
 */
/**
 * @typedef {object} Contender
 * @property {string} name
 * @property {(n: number) => Numbers} f64
 * @property {(n: number) => Numbers} i32
 * @property {(xs: unknown, vs: unknown, steps: number) => number} particles
 * @property {(pixels: unknown) => number} grayscale
 * @property {(n: number, steps: number) => number} nbody
 * @property {(n: number) => bigint} hash64
 * @property {(xs: unknown) => number} sorted
 * @property {(size: number) => number} mandel
 * @property {(text: string) => number} words
 * @property {(a: number, b: number) => number} add
 */
/**
 * @typedef {object} Kernels The kernels, as both contenders export them.
 * @property {(xs: never, vs: never, steps: number) => number} particles
 * @property {(pixels: never) => number} grayscale
 * @property {(n: number, steps: number) => number} nbody
 * @property {(n: number) => bigint} hash64
 * @property {(xs: never) => number} sorted
 * @property {(size: number) => number} mandel
 * @property {(text: string) => number} words
 * @property {(a: number, b: number) => number} add
 */
/** @typedef {new (n: number) => { ptr(): number, free(): void }} RustArray */
/**
 * @typedef {Kernels & {
 *   default: () => Promise<{ memory: WebAssembly.Memory }>,
 *   F64s: RustArray,
 *   I32s: RustArray,
 * }} RustModule What wasm-bindgen generates for rust/.
 */
/**
 * @typedef {Kernels & {
 *   memory: WebAssembly.Memory,
 *   newF64: (n: number) => number,
 *   newI32: (n: number) => number,
 *   dataF64: (a: number) => number,
 *   dataI32: (a: number) => number,
 *   release: (a: number) => void,
 * }} AsModule What asc generates for assemblyscript/.
 */

/**
 * The kernels of a module, behind the shape runner.js expects: what each
 * function takes is the handle its own arrays give.
 * @param {string} name
 * @param {Kernels} m
 * @param {(n: number) => Numbers} f64
 * @param {(n: number) => Numbers} i32
 * @returns {Contender}
 */
function contender(name, m, f64, i32) {
  return {
    name,
    f64,
    i32,
    particles: (xs, vs, steps) =>
      m.particles(/** @type {never} */ (xs), /** @type {never} */ (vs), steps),
    grayscale: (pixels) => m.grayscale(/** @type {never} */ (pixels)),
    nbody: (n, steps) => m.nbody(n, steps),
    hash64: (n) => m.hash64(n),
    sorted: (xs) => m.sorted(/** @type {never} */ (xs)),
    mandel: (size) => m.mandel(size),
    words: (text) => m.words(text),
    add: (a, b) => m.add(a, b),
  };
}

/**
 * Rust, through wasm-bindgen: `F64s` and `I32s` hold the arrays, `ptr()` says
 * where their elements are.
 * @returns {Promise<Contender>}
 */
async function rust() {
  const url = "./rust/bench_browser_rust.js";
  /** @type {unknown} */
  const loaded = await import(url);
  const m = /** @type {RustModule} */ (loaded);
  const wasm = await m.default();
  /**
   * @param {RustArray} Kind
   * @param {typeof Float64Array | typeof Int32Array} Typed
   * @returns {(n: number) => Numbers}
   */
  const array = (Kind, Typed) => (n) => {
    const a = new Kind(n);
    return {
      view: () => new Typed(wasm.memory.buffer, a.ptr(), n),
      handle: a,
      release: () => {
        a.free();
      },
    };
  };
  return contender("Rust", m, array(m.F64s, Float64Array), array(m.I32s, Int32Array));
}

/**
 * AssemblyScript, through its own bindings: an array is a pinned object of its
 * garbage collector, `dataF64` and `dataI32` say where its elements are.
 * @returns {Promise<Contender>}
 */
async function assemblyScript() {
  const url = "./as/kernels.js";
  /** @type {unknown} */
  const loaded = await import(url);
  const m = /** @type {AsModule} */ (loaded);
  /**
   * @param {(n: number) => number} make
   * @param {(a: number) => number} data
   * @param {typeof Float64Array | typeof Int32Array} Typed
   * @returns {(n: number) => Numbers}
   */
  const array = (make, data, Typed) => (n) => {
    const a = make(n);
    return {
      view: () => new Typed(m.memory.buffer, data(a), n),
      handle: a,
      release: () => {
        m.release(a);
      },
    };
  };
  return contender(
    "AssemblyScript",
    m,
    array(m.newF64, m.dataF64, Float64Array),
    array(m.newI32, m.dataI32, Int32Array),
  );
}

/** Every contender, loaded. */
export async function contenders() {
  return [await rust(), await assemblyScript()];
}
