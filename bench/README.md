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

`bench/browser/kernels.peras` holds eight kernels that a web application runs: particles moving in place, an image filter, gravity between structures, 64-bit hashing, sorting, floating-point arithmetic, splitting text, and a call from JavaScript. `bench/browser/reference.js` holds the same functions in JavaScript twice where it matters: as JavaScript is usually written (typed arrays for numbers, objects for structures, `BigInt` for 64-bit integers, `split` and `filter`), and as an expert writes it for speed (a loop over the characters, two 32-bit halves, columns of `Float64Array`, a radix sort). The particles, the pixels, Mandelbrot and the calls are already written as an expert would.

`node scripts/bench-browser.mjs` builds the kernels with the TLSF allocator, serves them, and runs them in Chromium next to their JavaScript twins. In the page, it first checks that every variant gives exactly the same result; then it times each one, alternating them: one cold call, 3 warm-up calls, 15 timed calls. It gives two ratios: against JavaScript as usually written, and against the fastest JavaScript. It also measures the download and the import of each module in a fresh browser context. `--check` only compares results, on small inputs, for CI. It writes [`browser/RESULTS.md`](browser/RESULTS.md).

What it measures, and what it does not:

- It measures an engine running WebAssembly against the same engine running JavaScript, in the same page, on the same inputs. Numbers move a lot between machines; the ratios move much less.
- The kernels were chosen to cover both sides: those where Peras should win and those where it should lose (very short calls, floating-point arithmetic alone). Every one is in the results.
- The JavaScript was written by the authors of Peras. The expert versions were added after an outside reader's question: would the figures hold against JavaScript written for speed? Some did not. Better JavaScript is welcome: a faster twin that gives the same result replaces ours, and the figures with it.
- It does not measure memory use, the garbage collector's pauses over a long session, or a whole application.

What the results say, today, against the **fastest** JavaScript (three runs, Chromium 153, a 4-core cloud machine); against JavaScript as usually written in brackets:

- **64-bit integers**: 5.4 times faster (20 times against `BigInt`). WebAssembly has 64-bit integers; JavaScript has to split them in two.
- **Numbers in arrays** (particles, pixels): 2.1 to 2.4 and 1.6 to 1.8 times faster, when the array is a `PerasArray`; 1.1 to 1.3 when a plain array is copied in and out. Bounds checks proven away and SIMD loops make the difference ([bounds.rs](../crates/perasc-ir/src/bounds.rs), [simd.rs](../crates/perasc-codegen-wasm/src/simd.rs)).
- **Sorting**: 1.8 to 1.9 times faster than a radix sort written in JavaScript (3.8 to 4.2 times faster than `Float64Array.prototype.sort` in V8, 17 times than `array.sort((a, b) => a - b)`). In WebKit, whose native sort is fast, Peras was slower than the native sort (0.88).
- **Text**: 1.6 to 1.7 times faster than a loop over the characters (3.2 to 3.3 times than `split` and `filter`). The steps are in [decision 0004](../docs/en/decisions/0004-strings-without-allocation.md).
- **Structures** (n-body): **the same speed** (0.97 to 1.01) as JavaScript storing the fields in columns of `Float64Array`, which is what Peras does; 1.5 times faster than an object per body.
- **Floating-point arithmetic alone** (Mandelbrot): the same speed (0.98 to 1.07).
- **Calls**: a call into the module costs 1.4 to 1.7 times a JavaScript call, which the engine inlines. Work should cross the boundary in large pieces.
- **Start**: 13 KiB gzip more to download, and the import takes about twice as long (19 ms against 9.5), mostly the allocator inlined in the loader. It is paid once per module.

What Peras brings, then, is less a speed JavaScript cannot reach than the expert's speed from the usual code: `text.split(" ").filter(…).length` and an array of structures run as fast as, or faster than, the JavaScript an expert rewrites them into.

### What an independent review would still ask

These are known and not done yet; each is a reason to read the figures above with care.

- **Other contenders**: the same kernels in Rust compiled to WebAssembly, and in AssemblyScript, the alternatives to a new language.
- **Every measure in the repository**: the prototypes and probes behind decisions 0004 and 0005, and the measures of floats as text and of `encodeInto`, were run from a scratch folder; they should be committed under `bench/` with one command each.
- **Statistics**: confidence intervals (bootstrap over the runs) rather than medians alone; more runs; the engine's tiers reported.
- **Machines**: macOS with Apple's Safari, Windows, an ARM phone; WebKit's timer counts whole milliseconds, too coarse for kernels of 2 or 3 ms.
- **A whole application**, measured from loading to the last frame.

Ideas that go against what was done so far, to measure before keeping them:

- **Compiling to JavaScript on purpose.** A small function called from JavaScript in a loop costs more at the boundary than its work (the calls above). The compiler could emit it as JavaScript, as the managed mode will, and keep WebAssembly for the rest: the choice made per function, by measure.
- **Letting the host sort where it sorts faster.** WebKit's native sort beats Peras's; a module could time both once at load on a small array and keep the faster, as the loader already probes for SIMD.
- **Floats as text in two paths.** A float with an integer value, the common case, can be written by the runtime's integer path; only the others need the host's `String()` (decision on floats as text, [methods](../docs/en/reference/methods.md)).
- **Maps of two layouts.** Swiss tables for maps whose order cannot be seen, insertion order for the others ([decision 0005](../docs/en/decisions/0005-maps-and-sets.md)).

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
