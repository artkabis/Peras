# Benchmarks

_French translation: [README.fr.md](README.fr.md)._

## Bug-catching benchmark (`bench/bugs`)

Each case is a known bug, or a rule of rigour, written twice: in idiomatic TypeScript (`.ts`) and in equivalent Peras (`.peras`). Three tools are asked whether they reject it:

1. `tsc` with the strictest options (`bench/bugs/tsconfig.json`);
2. the same, plus ESLint's recommended rules and typescript-eslint's strict type-aware preset (`bench/bugs/eslint.config.js`);
3. `perasc check`.

Run it with `node scripts/bench-bugs.mjs`, after `pnpm install`; it writes [`bugs/RESULTS.md`](bugs/RESULTS.md).

What it measures, and what it does not:

- It measures whether well-known pitfalls of JavaScript and TypeScript are rejected before the code runs.
- The corpus was written by the Peras authors, around rules Peras enforces: it shows coverage of these pitfalls, not general superiority. Cases where TypeScript already does as well are kept on purpose.
- TypeScript has no `u8` or integer types, so case 08 uses a `Uint8ClampedArray`, and case 19 has no TypeScript equivalent at all.
- It does not measure speed: the browser benchmark below does.

`bench/bugs` holds deliberately wrong code: the repository's ESLint and Prettier ignore it.

## Backend benchmark (`bench/backends`)

`bench/backends/kernels.peras` holds four kernels: recursion, integer loops, float loops and 64-bit arithmetic. `node scripts/bench-backends.mjs` builds them with both code generators (direct WebAssembly, and Rust through `cargo` and `wasm-bindgen`), checks that every build returns the same results as a JavaScript reference, then measures build time, module size (raw, gzip, after `wasm-opt -O3`) and run time in Node. A direct build with `--no-optimize` shows what the IR optimizations bring; `--check` only compares results, for CI. It needs `rustup` and `wasm-bindgen` 0.2.130 for the Rust backend, and writes [`backends/RESULTS.md`](backends/RESULTS.md). Decision [0001](../docs/en/decisions/0001-webassembly-backend.md) discusses the results.

## Browser benchmark (`bench/browser`)

`bench/browser/kernels.peras` holds eight kernels that a web application runs: particles moving in place, an image filter, gravity between structures, 64-bit hashing, sorting, floating-point arithmetic, splitting text, and a call from JavaScript. `bench/browser/reference.js` holds the same functions in JavaScript, written as a developer who cares about speed would: typed arrays for numbers, plain objects for structures, `BigInt` for 64-bit integers.

`node scripts/bench-browser.mjs` builds the kernels with the TLSF allocator, serves them, and runs them in Chromium next to their JavaScript twins. In the page, it first checks that every variant gives exactly the same result; then it times each one, alternating them: one cold call, 3 warm-up calls, 15 timed calls. It also measures the download and the import of each module in a fresh browser context. `--check` only compares results, on small inputs, for CI. It writes [`browser/RESULTS.md`](browser/RESULTS.md).

What it measures, and what it does not:

- It measures V8 running WebAssembly against V8 running JavaScript, in the same page, on the same inputs. Numbers move a lot between machines; the ratios move much less (under 10 % between two runs).
- The kernels were chosen to cover both sides: those where Peras should win and those where it should lose (text, very short calls). Every one is in the results.
- The JavaScript is fair but not heroic: no hand-written WebAssembly, no SIMD, no Web Workers, on either side.
- It does not measure memory use, the garbage collector's pauses over a long session, or a whole application.

What the results say, today:

- **Numbers in arrays** (particles, pixels): Peras is 1.8 to 2.2 times faster when the array is a `PerasArray`. A counted loop drops the bounds checks it proves (`crates/perasc-ir/src/bounds.rs`), which took the particles from 1.2 to 1.4; its turns then run several at a time with WebAssembly SIMD, branches included (`crates/perasc-codegen-wasm/src/simd.rs`), which took the particles to 1.8 and the grayscale filter from 1.2 to 2.2. Passing a plain array copies it in and out, which costs more than the work: zero copy is what makes the gain.
- **Floating-point arithmetic alone** (Mandelbrot): the same speed (0.98 to 1.07 between runs). V8 already compiles such JavaScript very well, and Peras checks every index and every integer operation.
- **Structures** (n-body): 1.5 to 1.8 times faster. An array of structures is stored by columns in linear memory, where JavaScript has an object per element.
- **64-bit integers**: about 20 times faster. JavaScript has only `BigInt`, which allocates.
- **Sorting**: 4.2 times faster than `Float64Array.prototype.sort`, which V8 runs as native code, and 17 times faster than the usual `array.sort((a, b) => a - b)`. The runtime sorts numbers by their bits, a radix sort in a few passes, where a comparison sort needs about 20 comparisons per number; it was a heapsort, 2.3 times slower than V8, before.
- **Text**: 1.15 times slower; it was 3.4. The parts of `split` read at once are views of the text, one block for them all ([decision 0004](../docs/en/decisions/0004-strings-without-allocation.md)). Measured step by step, crossing into the module in UTF-8 costs 4 ms of the 17; the rest is allocation, a block per word. `filter` on a temporary array now keeps its elements in place instead of copying them, a short string is one block instead of two, and `indexOf` looks for the first byte of the needle before comparing the rest. Text that is only split and counted should still stay on the JavaScript side.
- **Calls**: a call into the module costs about 1.4 times a JavaScript call, which V8 inlines. Work should cross the boundary in large pieces.
- **Start**: about 11 KiB gzip more to download, and the import takes about twice as long (17 ms against 9), mostly the allocator inlined in the loader. It is paid once per module.

## A/B benchmark (`scripts/bench-ab.mjs`)

Two builds of Peras, compared on the kernels of `bench/browser` in one Chromium page, so that a change is decided on measures:

```sh
node scripts/bench-ab.mjs --a ref:HEAD --b current           # a commit against the working tree
node scripts/bench-ab.mjs --a current --a-flags "--simd off" --b current   # an option
```

A and B are each `current` (the working tree), `ref:<git ref>` (built in a worktree under `target/bench-ab`) or `bin:<path>`, with their own perasc options. Each of 4 rounds is a fresh page where A, B and JavaScript alternate, 30 timed calls after 3 of warm-up; A runs first in odd rounds and B in even ones, so that neither takes every coldest first call. Every variant must give the same result and leave nothing alive. Each build's runtime counts the allocations of one call of each variant. Then the size, the import time in a fresh context and the compile time of each build. The report goes to `target/bench-ab/REPORT.md`; `--check` only checks that both builds agree, for CI.

The report applies these thresholds:

| Indicator              | Threshold                                                                                    |
| ---------------------- | -------------------------------------------------------------------------------------------- |
| A kernel's median      | a gain counts at 10 % or more in **every** round, a regression at 3 % or more in every round |
| Allocations per call   | must not grow                                                                                |
| Leaks, double releases | none, checked by the page                                                                    |
| Download               | at most +1 KiB gzip for one change                                                           |
| Compile time           | at most +10 %                                                                                |

They were calibrated by comparing a build with itself: every kernel stayed between −2.9 % and +1.8 %, the compile time within +4.8 %. On this machine, separate runs of the browser benchmark differ by up to 10 %; the same page keeps it under 3 %.

## Other engines, and real devices

Both browser scripts take `--browser chromium|firefox|webkit`, Chromium by default. WebKit is Safari's engine, JavaScriptCore, as Playwright builds it for Linux: close to Safari, not Apple's own build. Another engine's results go to `target/bench-browser/RESULTS-<engine>.md`; the committed `RESULTS.md` stays Chromium's. CI runs both benchmarks in WebKit and puts their reports in the run's summary.

To measure a real Safari, on a Mac or an iPhone, `--serve` builds the page and serves it instead of driving a browser; the page shows its results itself:

```sh
node scripts/bench-ab.mjs --a current --a-flags "--simd off" --b current --serve --host 0.0.0.0 --port 8080
# then open http://<this machine>:8080/index.html?first=A, and ?first=B, on the device
```

`--host 0.0.0.0` serves the folder to the local network, for a phone; stop the script when done. Until the measures of an engine are in, nothing is claimed about it: the figures in this README are Chromium's.
