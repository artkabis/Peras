# 0031: Parallel loops that can draw a frame

_French translation: [../../fr/decisions/0031-useful-parallel-loops.md](../../fr/decisions/0031-useful-parallel-loops.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks. It answers finding F301, which [decision 0029](0029-pond-of-lanterns.md) met again: the pond cannot be drawn by a `parallel for`, and on that frame only threads can take Peras past JavaScript (its profile, finding F222)._

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

## Record

_Each part is recorded here as it lands._
