# 0031: Parallel loops that can draw a frame

_French translation: [../../fr/decisions/0031-useful-parallel-loops.md](../../fr/decisions/0031-useful-parallel-loops.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks (commit `3a1ba4a`); options measured at `29b742f`; option p2 accepted by the owner on 2026-11-17. It answers finding F301, which [decision 0029](0029-pond-of-lanterns.md) met again: the pond cannot be drawn by a `parallel for`, and on that frame only threads can take Peras past JavaScript (its profile, finding F222)._

## Context

What the compiler does today, read before writing this (finding M020):

- **A `parallel for` is a job** ([decision 0007](0007-threads-over-shared-memory.md), `crates/perasc-ir/src/lower/parallel.rs`): its body becomes a function, `f__par0(start, end, captures…)`, which the loader's workers run over ranges of turns, on one shared memory. Each worker instantiates the module with no runtime: a job never allocates.
- **What a turn may not do** (`check`, `E0241`): assign a variable declared outside the loop; write an element at another index than its counter's (`Index`); read, at another index, an array the loop writes (`Reads`); call a function (`Calls`); make, move or release a buffer (`Heap`); read or write a module variable, each thread having its own (`Global`); leave the loop.
- **What the pond needs**: its sky calls `hash`, `unit` and `tri`, which neither allocate nor touch a module variable; a lantern fills pixels across the screen, so a turn split by lanterns would write anywhere, but a turn split by bands of rows writes only its band, and reads the depth it wrote there. Split by rows, each pixel sees the same operations in the same order as on one thread: the frame stays the same, bit for bit.
- **JavaScript has threads too**: Web Workers over a `SharedArrayBuffer`. A Peras on threads is compared with a TypeScript on workers, not only with the TypeScript on one thread.

## Options

- **p0. As today**: the frame cannot be written as a `parallel for`; the pond runs on one thread.
- **p1. Pure calls in a turn**: a turn may call a function that, with every function it calls, makes, moves and releases no buffer, reads and writes no module variable, writes through no parameter, and calls no function value. The checker proves it from the call graph; a call that is not pure stays refused, its reason named.
- **p2. p1, and parts**: in a turn counted by `b`, `xs.part(b, k)` is the elements `b * k` to `(b + 1) * k` of `xs`, which the turn may read and write; the parts of two turns never overlap, by construction. A part is a view: nothing is copied, and it cannot leave the turn. The pond's turn takes the part of its band in `light`, `depth` and `pixels`.
- **p3. p1, and ranges proven**: no new notion; the checker proves that every index a turn writes or reads in an array it writes lies in a range of its own, `[b * k, (b + 1) * k)`, from the bounds of its loops. Built only if it can be within this decision; otherwise reported as not built, with why (finding M001).

## Protocol, registered before any prototype

Four quantities, as for decisions 0027, 0028 and 0030. Each option is prototyped behind `PERAS_PROTO_PAR`; the build without it keeps p0.

1. **Ease of writing**: four programs, written as a user writes them from the TypeScript on one thread, then fixed until they build, for each option:
   - P1, the pond, frame 600 split into bands of rows;
   - P2, the garden of decision 0013, split into rows;
   - P3, a blur of an image, each row of the output from three rows of the input;
   - P4, the dashboard of decision 0014 split into chunks of lines, which allocates and sums: expected refused by every option, the refusal reported.

   For each: the errors met before it builds, with their codes, the lines changed from the program on one thread, the notions JavaScript does not ask for.

2. **The same results**: every program built, on threads, gives the bytes it gives on one thread and the TypeScript gives, checked by a hash.
3. **Workloads**, in Chromium, the page cross-origin isolated, 15 runs after 2, the versions alternating, the ratio round by round to Peras on one thread (finding M017): W1, the pond, frame 600 at 1024 × 768 with 1,000 lanterns; W2, the garden at 1024 × 1024; W3, the blur of a 2048 × 2048 image. Each against the TypeScript on one thread and the TypeScript on as many workers, split the same way, its memory a `SharedArrayBuffer`.
4. **Load on the processor**: the CPU time of every thread over the wall time, as decision 0012 measures it: threads finish sooner and keep more cores busy, and both are reported.
5. **Memory and bytes**: the memory of each module and of the workers; the bytes of the module and its loader, raw and gzip.
6. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-parallel.mjs` runs them; `--check` checks that every program built gives its bytes on one thread, and is run with every check of the project.

## Decision

**Option p2: pure calls, and parts.**

1. A turn of `parallel for` may call a function that, with every function it calls, makes, moves and releases no buffer, writes no element, reads and writes no module variable, and calls no function value; the checker proves it from the call graph, and names the reason of a call it refuses.
2. In a turn counted by `b`, `let band = xs.part(b, k)` is the elements `b * k` to `(b + 1) * k` of `xs`, which the turn may read and write through `band`; `k` must not depend on the turn. Parts of two turns never overlap. A part is a view: nothing is copied, and it cannot leave the turn. An array written through parts is written and read only through them in the turn.
3. The loader splits a loop of two turns already when its turn holds a loop or a call, a heavy turn; a loop whose turn does neither keeps waiting for 256 turns, waking the workers costing more than such turns.
4. Options p0 and p1 stay buildable behind `PERAS_PROTO_PAR`, for the measure script only.

## Criteria of the step, registered before its code

1. **Build tests**: a page of loops that build, a pure call, a call returning a structure, parts in two arrays, a part read where it is written; and refused with `E0241` and their reason: a part not taken at the counter, a part whose size the turn computes, an array written through a part and at the counter, a call to a function that writes a module variable, a call to one that allocates, a call through a function value, a part used outside a turn.
2. **The same results**: the interpreter's tests of `parallel for` gain a pure call and parts, unoptimized and optimized, against the loop on one thread; `node scripts/bench-parallel.mjs --check` gives the TypeScript's bytes on four threads for the pond, the garden and the blur.
3. **Each rule broken on purpose is caught**: a part accepted at another index than the counter, and a call accepted to a function that writes a module variable, each fail a build test.
4. **Measures**, reported, not conditions: `node scripts/bench-parallel.mjs` at the commit of the step; `node scripts/bench-workers.mjs`, whose loops of one element a turn keep the threshold of 256.
5. **Docs**, in English and in French: the guide's page on threads, the methods reference for `part`, `E0241`'s page, the status, findings F301 and F015, the roadmap, the changelog, `CLAUDE.md`.

## Record

_Each part is recorded here as it lands._

- **2026-11-17, the options measured** (`node scripts/bench-parallel.mjs`, [results](../../../bench/parallel/RESULTS.md); prototypes at `10f7efe`, behind `PERAS_PROTO_PAR`; run 2 at `29b742f`). An Intel Xeon at 2.30 GHz, 4 cores, Chromium headless, the page cross-origin isolated, 4 threads. Every version, on threads or not, gives the TypeScript's bytes, checked by SHA-256.
  - **Ease of writing**: each program as first written on threads is refused by every option (`E0241`); the version that builds:

    | Program        | p0, today                   | p1, pure calls             | p2, and parts           |
    | -------------- | --------------------------- | -------------------------- | ----------------------- |
    | P1, the pond   | none                        | none                       | in bands, 102 lines     |
    | P2, the garden | none: it calls              | one turn a pixel, 11 lines | one turn a row, 3 lines |
    | P3, the blur   | one turn a pixel, 29 lines  | the same                   | one turn a row, 8 lines |
    | P4, dashboard  | none: it allocates and sums | none                       | none                    |

    p1 cannot draw the pond: without parts, a turn writes only its own index, and a pixel would have to look at all 88,000 triangles. The blur builds with p0 once its turn makes no array, `for (const row of [up, y, down])` written as three lines.

  - **Speed**, the median of the ratios round by round, 15 runs:

    | Workload                                 | TypeScript, 1 thread | TypeScript, 4 workers |  Peras, 1 thread | Peras on threads                            |
    | ---------------------------------------- | -------------------: | --------------------: | ---------------: | ------------------------------------------- |
    | W1, the pond, 1024 × 768, 1,000 lanterns |              54.3 ms |    27.0 ms (0.497 TS) |  52.5 ms (0.934) | p2: 25.5 ms, **0.424 of TypeScript's**      |
    | W2, the garden, 1024 × 1024              |             766.6 ms |   313.2 ms (0.406 TS) | 862.4 ms (1.128) | p1: 251.5 ms, 0.325; p2: 259.0 ms, 0.337    |
    | W3, the blur, 2048 × 2048                |              48.3 ms |    13.8 ms (0.290 TS) |  93.8 ms (1.946) | p0 and p1: 7.3 ms, 0.151; p2: 4.3 ms, 0.090 |

  - **Load on the processor**, every thread's CPU time over the wall time: 1.0 on one thread, 2.6 to 3.8 on four, TypeScript's workers and Peras's alike: threads finish sooner by keeping more cores busy.
  - **Memory and bytes**: the same buffers for every version (27, 4 and 64 MiB); Peras on threads ships 6.5 to 10.9 KB gzipped, its loader carrying the pool of workers, against 4.3 to 8.2 KB on one thread; TypeScript 0.2 to 2.1 KB.
- **Departures from the protocol.**
  - **The loader splits a loop of two turns already** in p1 and p2, where it waited for 256: a turn may be a band of rows. Without it, the pond's 4 bands ran on the page alone (77 ms, seen before the run).
  - **The number of bands**, which the protocol left open, is 4, the cores, chosen after informal runs of 4 to 48 bands; the pond's rows of 1 per turn were not tried.
  - **The blur on threads makes no array per pixel**, where the TypeScript and Peras on one thread do: Peras on one thread pays an allocation a pixel (finding F223), so its ratio to Peras on threads overstates what threads give; against the TypeScript, which V8 keeps from allocating, the comparison holds.
  - **Run 1 is kept with two flaws** in [run-1](../../../bench/parallel/run-1/RESULTS.md): the bytes of the TypeScript versions left out their module, and the memory of the TypeScript on one thread its typed arrays (finding M006); corrected for run 2, the times alike.
  - **A bug found and fixed in the prototype** (finding F015): a call returning several results in a turn kept its result locals out of the job.
- **What the measures say**: only p2 draws the pond, and on it Peras on threads takes 0.424 of the TypeScript's time, 2.4 times as fast, and a little less than the TypeScript on four workers (0.497). On the garden, p1 and p2 are alike, 3 times the TypeScript's speed and 1.2 times its workers'. On the blur, p2 is the fastest, 11 times the TypeScript's speed. p2 is also the easiest where a frame splits into rows: 3 and 8 lines changed. p2, with p1's pure calls in it, is the option to propose to the owner, who chooses.
