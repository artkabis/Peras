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

## Web application bugs (`bench/web-bugs`)

`node scripts/bench-web-bugs.mjs` takes seventeen bugs of web applications, each written as it is naturally written in TypeScript and in a managed Peras module, and finds the earliest point where the toolchain stops each: at compile time (`tsc` at its strictest, then typescript-eslint's strict type-checked preset; `perasc check`), at run time by an error, in Chromium, or not at all ([results](web-bugs/RESULTS.md), [decision 0011](../docs/en/decisions/0011-what-to-build-next.md)). Peras runs as a debug build, as code runs while it is written and tested. CI checks that Peras lets no more through than decision 0011 recorded.

| Stopped                     | TypeScript | Peras |
| --------------------------- | ---------: | ----: |
| at compile time             |          7 |     9 |
| at run time, with a message |          1 |     6 |
| silent                      |          8 |     1 |

Peras alone stops the listeners and timers left running and the libraries that lie about their types; TypeScript alone stops an index past the end at compile time, and keeps a total above 2³¹ exact, which a debug build of Peras stops with a message where an `i32` cannot hold it, and a production build wraps. The corpus was written by the Peras authors, both versions as naturally as they could: it covers the bugs of web applications they know, not every bug.

## Case study (`bench/case-study`)

`node scripts/bench-case-study.mjs` runs a table of orders, searched after a delay, sorted, with a summary, written in TypeScript ([orders.ts](case-study/orders.ts)) and in a managed Peras module ([orders.apeiron](case-study/orders.apeiron)), in Chromium: the same rows shown, the time to mount and to sort, nothing reacting once removed, two tables in one page each with its own state, and what each costs the machine: bytes shipped, CPU time, memory ([decision 0012](../docs/en/decisions/0012-cost-on-the-machine.md)) ([results](case-study/RESULTS.md)). A third version sorts through an `extern` block, to measure what a comparison sort in Peras would change. Decision 0011 lists what writing it in Peras ran into.

## Demo (`bench/demo`)

`node scripts/demo.mjs` runs the same animation of particles in three versions, alone in Chromium for 10 s each, and puts their recordings side by side, the GIF at the top of the README ([results](demo/RESULTS.md)): TypeScript as it is often written, a new object per particle at each frame; TypeScript written for speed, typed arrays by hand; and Peras compiled to WebAssembly. Each draws its frame rate and the memory the page holds, on one scale. At 1,500,000 particles: 8, 47 and 58 frames a second; the first one's heap climbs past 1 GB and falls at each collection, the other two stay flat at the same 50 MB. The gap between the TypeScript versions is the garbage collector's, the gap to Peras WebAssembly's; a managed module runs at TypeScript's speed. CI checks that the three run and draw.

## Data at scale (`bench/data`)

`node scripts/bench-data.mjs` measures a sales dashboard under a protocol registered before any measure ([decision 0014](../docs/en/decisions/0014-data-at-scale.md), [results](data/RESULTS.md)): a generated CSV of 100,000 to 4,000,000 rows, in idiomatic TypeScript, TypeScript optimized by hand and Peras, which give the same summary byte for byte; the time to the summary warm, to the first chart cold, the peak memory of the renderer, the bytes shipped and the lines of code. Two hypotheses held, five were refuted: plain Peras is slower than plain TypeScript. CI checks that the versions give the same summary.

## The garden of Apeiron (`bench/garden`)

`node scripts/bench-garden.mjs` measures a procedural scene under a protocol registered before any measure ([decision 0013](../docs/en/decisions/0013-garden-of-apeiron.md), [results](garden/RESULTS.md)): a sky, mountains and a meadow drawn pixel by pixel, written in TypeScript, Peras and Rust, which give the same pixels bit for bit; the bytes of the program against PNG, WebP and AVIF of its renders, by FFmpeg's SSIM and PSNR; the time of a frame in Chromium. Four hypotheses held, one was refuted: Peras renders it 1.19 times as fast as TypeScript, short of the 1.3 times registered. The [first run](garden/run-1/RESULTS.md) is kept with the flaw of its method, and the correction is listed under the protocol. CI checks that the versions give the same pixels.

## Sorting (`bench/sort`)

`node scripts/bench-sort.mjs` sorts records by a number and by a string with `sortBy` and `sortByDescending`, in a managed module and in an owned one, against what TypeScript writes, `sort((a, b) => a.amount - b.amount)`, in Node ([source](sort/kernels.apeiron), [results](sort/RESULTS.md)). Each side makes the same records and must give the same order; the time of making them is subtracted. By a number, Peras sorts 3 to 7 times as fast as TypeScript from 10,000 records, in both modes: a radix sort of the keys' bits, carrying the elements' indices, where TypeScript calls a function at each comparison. By a string, a managed module is as fast as TypeScript, an owned one 1.1 to 1.4 times. CI checks the three agree, without a leak, and that the managed sort by a number is not slower.

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
- **Start**: 8.8 KiB gzip for the eight kernels, against 2.4 for their JavaScript, and the import takes about 1.7 times as long (18 ms against 11 here). A module carries only the runtime functions it calls, its embedded runtime pruned to them, and only the loader helpers its functions use: 13.3 KiB before for these kernels (−34 %), and for a small module, a function over an array or a text, 4.5 to 5.7 KiB against 11.3 to 11.5 (−49 to −59 %). The import time did not move with the bytes: what is left is compiling and instantiating two modules, the runtime and the program, and a second request, the `.wasm` asked for once the loader runs. One module instead of two, and a hint that fetches the `.wasm` with the loader, are the next steps.

What Peras brings, then, is less a speed JavaScript cannot reach than the expert's speed from the usual code: `text.split(" ").filter(…).length` and an array of structures run as fast as, or faster than, the JavaScript an expert rewrites them into.

### Against Rust and AssemblyScript

`node scripts/bench-browser.mjs --contenders` also builds the kernels in Rust ([`browser/rust`](browser/rust/src/lib.rs), wasm-bindgen 0.2.130) and in AssemblyScript ([`browser/assemblyscript`](browser/assemblyscript/kernels.ts), 0.28.20), the two other ways to write WebAssembly for the web, and runs them in the same page. Each is written as that language is usually written: a structure or a class per body, `split` and `filter`, the standard library's sort. Each is built for speed: Rust in release with LTO and SIMD, AssemblyScript at `-O3` with SIMD allowed. Their arrays stay in their module's memory, as a `PerasArray` does. Every variant gives exactly the same result, checked in the page; CI checks it on every pull request.

Three runs, Chromium 153; the contender's time divided by Peras's, above 1 when Peras is faster:

| Kernel    | Rust / Peras | AssemblyScript / Peras |
| --------- | -----------: | ---------------------: |
| particles |  1.49 – 1.52 |              4.8 – 4.9 |
| grayscale |  1.28 – 1.33 |              2.4 – 2.5 |
| sort      |  1.30 – 1.43 |              5.1 – 5.5 |
| words     |  1.44 – 1.48 |                    7.3 |
| nbody     |         0.98 |                    3.3 |
| hash64    |         1.00 |             1.0 – 1.04 |
| mandel    |  0.94 – 0.98 |            0.98 – 1.01 |
| calls     |  1.02 – 1.05 |             1.0 – 1.02 |

Download, gzip: Peras 8.8 KiB (13.1 before its runtime was pruned to what a module calls), Rust 16.1 KiB, AssemblyScript 6.1 KiB.

What it says:

- **Where the code is the same, so is the speed.** n-body, hashing, Mandelbrot and the calls give Rust and Peras the same machine code, within 6 %: Peras's WebAssembly is as good as LLVM's on them.
- **Where Peras is ahead of Rust, its compiler does more for the usual code**, not better code generation: the particles' loop with a branch runs four lanes under a mask, the sort is a radix sort where Rust's standard library compares, and Rust's time for `words` includes wasm-bindgen's crossing of the text, which copies ASCII characters one at a time before `encodeInto`. Why grayscale is 1.3 times faster in Peras, both being vectorized, is not analysed yet.
- **This is idiomatic Rust, not an expert's.** An expert would use `std::arch` intrinsics, a radix sort crate such as `radsort`, and pass the text as bytes: that Rust has not been measured, and would likely close the gaps above. The fair reading is "as fast as Rust, from code written as TypeScript is", not "faster than Rust".
- **AssemblyScript** is 2.4 to 7.3 times slower where it allocates or loops over arrays: its optimizer does not vectorize, its typed arrays check every index, its objects and strings go through its garbage collector. It is the smallest download. An expert would write `unchecked()` and `StaticArray`; not measured either.

### Web Workers

`node scripts/bench-workers.mjs` measures what splitting a kernel across Web Workers brings ([results](browser/WORKERS.md)). A module built by `perasc` runs in a module worker as it is: each worker loads its own instance, with its own memory. The page cuts the data into N parts, transfers them to N workers, each copies its part into its module's memory, runs the kernel, copies the result back and transfers it; the page gathers the parts. Everything is counted, against the kernel called directly on the main thread, its arrays already in the module's memory. Every split result is checked equal to the direct one, element by element; CI runs the check.

On 4 cores, Chromium 153, over 2 to 4 runs:

| Kernel               | Direct     | 4 workers  |      Speed-up |
| -------------------- | ---------- | ---------- | ------------: |
| particles, 200 steps | 243–263 ms | 86–100 ms  | **2.6 – 2.9** |
| words                | 2.9–3.8 ms | 2.5–3.2 ms |     1.1 – 1.2 |
| particles, 10 steps  | 13–18 ms   | 18–27 ms   |   0.64 – 0.73 |
| grayscale            | 4.0–4.9 ms | 21–29 ms   |   0.17 – 0.19 |

What it says:

- **Workers pay where the work outweighs moving the data.** 200 steps over a million particles, twenty times the work of 10 for the same bytes, run 2.6 to 2.9 times faster on 4 cores. One step of a filter over 16 MB of pixels runs 5 times slower: cutting, transferring, copying in and back out cost about 20 ms, the filter 4.
- **What is missing is shared memory, and its ceiling is measured.** Each worker has its own memory, so data goes in and out by copy. A last way, _4 workers, data resident_, keeps each part in its worker's memory and times only the kernel and the answers: the best that WebAssembly threads over one `SharedArrayBuffer` could do. Grayscale goes from 0.32–0.35 to 1.84–2.02 times the direct speed, 10 steps of particles from 0.80–0.83 to 2.08–2.22, 200 steps from 2.7–3.0 to 3.0–3.3 (3 runs, on a day this machine ran the direct kernels about twice slower). [Decision 0007](../docs/en/decisions/0007-threads-over-shared-memory.md) builds them, split by ownership; a page using them must be served with the COOP and COEP headers.
- **`parallel for` comes close to that ceiling.** `bench/browser/parallel.peras` writes the particles and grayscale loops `parallel for`, built with `--threads on`; the page is served cross-origin isolated. On 4 threads, particles run 1.95–2.23 times faster than the direct call for 10 steps, 2.68–2.81 for 200, grayscale 1.70–1.87, its checksum computed by a second loop on one thread (2 runs). Every result is checked equal to the direct one, element by element.
- **None of this is particular to Peras**: JavaScript, Rust or AssemblyScript in workers pay the same copies.

### What an independent review would still ask

These are known and not done yet; each is a reason to read the figures above with care.

- **Expert contenders**: Rust and AssemblyScript are measured as they are usually written (above); their experts' versions are not.
- **Every measure in the repository**: the prototypes and probes behind decisions 0004 and 0005, and the measures of floats as text and of `encodeInto`, were run from a scratch folder; they should be committed under `bench/` with one command each, as [`bench/prototypes`](prototypes/outer-simd/README.md) now is.
- **Statistics**: confidence intervals (bootstrap over the runs) rather than medians alone; more runs; the engine's tiers reported.
- **Machines**: macOS with Apple's Safari, Windows, an ARM phone; WebKit's timer counts whole milliseconds, too coarse for kernels of 2 or 3 ms.
- **A whole application**, measured from loading to the last frame.

Ideas that go against what was done so far, to measure before keeping them:

- **Compiling to JavaScript on purpose.** A small function called from JavaScript in a loop costs more at the boundary than its work (the calls above). The compiler could emit it as JavaScript, as the managed mode will, and keep WebAssembly for the rest: the choice made per function, by measure.
- **Letting the host sort where it sorts faster.** WebKit's native sort beats Peras's; a module could time both once at load on a small array and keep the faster, as the loader already probes for SIMD.
- **Floats as text in two paths.** A float with an integer value, the common case, can be written by the runtime's integer path; only the others need the host's `String()` (decision on floats as text, [methods](../docs/en/reference/methods.md)).
- **Maps of two layouts.** Swiss tables for maps whose order cannot be seen, insertion order for the others ([decision 0005](../docs/en/decisions/0005-maps-and-sets.md)).

## Maps and sets (`bench/maps`)

`node scripts/bench-maps.mjs` runs [bench/maps/maps.peras](maps/maps.peras), compiled with the TLSF allocator, against the same work on JavaScript's `Map` and `Set` with number keys, in Node ([results](maps/RESULTS.md)). Each case is checked to give the same result first; CI runs the check.

| Case                                | JavaScript | Peras      |        Speed-up |
| ----------------------------------- | ---------- | ---------- | --------------: |
| count 1 M words, `Map<String, i32>` | 235–262 ms | 200–208 ms | **1.13 – 1.31** |
| 2 M integer keys set, read, deleted | 203–212 ms | 165–174 ms | **1.20 – 1.24** |
| 2 M distinct integers, `Set<i32>`   | 109–112 ms | 104–110 ms |     1.02 – 1.06 |

The first compiled maps were slower than JavaScript's: 0.68, 0.93 and 0.69. Two changes, each measured:

- **SipHash in locals.** The runtime kept SipHash's state in globals, which the engine does not keep in registers: each round went through memory. Its rounds now work on locals, written out: integer keys go from 222 to 167 ms, the set from 150 to 112, words from 378 to 313.
- **`m.set(word.copy(), …)` lends the word**, which the map copies only when the key is new: the copy made then released at each word already there is gone. Words go from 313 to 201 ms.
- **Tried and dropped**: a cache of the entry the last lookup found, for the `get` then `set` of the same key. Words did not move (313 to 308 ms), and the comparison it adds to every lookup slowed integer keys to 191 ms and the set to 128.

## Managed modules (`bench/managed`)

`node scripts/bench-managed.mjs` compiles [bench/managed/kernels.apeiron](managed/kernels.apeiron) to JavaScript for production, and the same source to WebAssembly as a `.peras` file with the TLSF allocator, and runs both against the same work written by hand in JavaScript, in Node ([results](managed/RESULTS.md), [decision 0008](../docs/en/decisions/0008-managed-mode.md)). Each case is checked to give the same result on the three sides first; CI runs the check. Speed is the time of hand-written JavaScript divided by the module's, over 6 runs:

| Case                                      | Managed (JavaScript) | Owned (WebAssembly) |
| ----------------------------------------- | -------------------: | ------------------: |
| sum of 1 M `i32`, wrapping                |          0.99 – 1.01 |         1.09 – 1.14 |
| one step of 1 M particles, `inout` arrays |          0.94 – 1.03 |         0.16 – 0.18 |
| count 500 k words, `Map<String, i32>`     |          0.96 – 1.31 |         1.67 – 1.91 |
| energy of 2,000 bodies, structures        |          0.93 – 0.99 |         1.02 – 1.10 |

A managed module runs at the speed of the JavaScript one would write: its numbers wrap as hand-written code wraps them, and the bounds checks of `xs[i]` are proven away in a counted loop over `xs` whose body calls nothing, as in WebAssembly; before that proof, the sum ran at 0.83 – 0.86 and the particles at 0.89 – 0.93. The particles' arrays are JavaScript arrays, copied into the owned module and back at each call: a `PerasArray` avoids it ([arrays without a copy](../docs/en/guide/javascript.md#arrays-without-a-copy)).

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
