# 0007: Threads over shared memory, split by ownership

_French translation: [../../fr/decisions/0007-threads-over-shared-memory.md](../../fr/decisions/0007-threads-over-shared-memory.md)._

## Context

A web page has several cores, and a Peras module uses one. The [Web Workers benchmark](../../../bench/README.md#web-workers) runs a kernel in N workers today, each with its own instance of the module and its own memory. The data goes in and out by copy, and the copies decide the outcome:

- 200 steps over a million particles run 2.7 to 3 times faster on 4 cores;
- one pass of a filter over 16 MB of pixels runs 3 times slower: copying costs about 20 ms, the filter 4 to 8.

WebAssembly threads remove the copies. Several workers instantiate the same module over one `WebAssembly.Memory({ shared: true })`, backed by a `SharedArrayBuffer`, and read the same arrays. Before building that, this decision measures what it can bring at best.

## What others have done

- **Rust and C++.** `wasm-bindgen-rayon` and Emscripten's pthreads instantiate the module once per worker over one shared memory. Rayon splits a slice into disjoint parts that the borrow checker proves independent, and steals work between threads.
- **The cost of a page.** A `SharedArrayBuffer` is only given to a page that is cross-origin isolated: served with `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`, since Spectre (2018). Every resource of such a page must allow it; some third-party scripts and embeds do not.
- **What shared memory costs.** It cannot shrink. Growing it must be seen by every thread. The main thread may not block on `Atomics.wait`. An allocator shared between threads needs locks or one arena per thread.

## Measurement

`node scripts/bench-workers.mjs` adds a way: _4 workers, data resident_. Each worker keeps its part in its module's memory, put back as it was before each run, untimed. Only running the kernel on the 4 workers and hearing back from each is timed. That is the time of threads over one shared memory with nothing copied, the cost of the shared allocator and of synchronization left out: a ceiling. Every result is checked equal to the direct one, element by element.

On 4 cores, Chromium 153, 3 runs of 15 medians each:

| Kernel               | Direct     | 4 workers, copies | 4 workers, resident (ceiling) |
| -------------------- | ---------- | ----------------: | ----------------------------: |
| particles, 10 steps  | 19–20 ms   |       0.80 – 0.83 |               **2.08 – 2.22** |
| particles, 200 steps | 325–334 ms |       2.68 – 2.99 |               **3.00 – 3.28** |
| grayscale            | 8.1–8.7 ms |       0.32 – 0.35 |               **1.84 – 2.02** |

Speed-ups against the direct call on the main thread. This machine was slower that day than for the first Workers measure (grayscale direct 8 ms instead of 4); the ratios hold.

What it says:

- **Without copies, light kernels gain too.** Grayscale goes from 3 times slower to 2 times faster; 10 steps of particles from 0.8 to 2.2 times.
- **Heavy kernels gain little more.** 200 steps already paid for their copies: 3 to 3.3 instead of 2.7 to 3.
- **4 cores do not give 4 times.** A filter over 16 MB is bound by the memory's bandwidth, which the cores share, and waking 4 workers costs a fraction of a millisecond.

## Decision

1. **Peras builds threads over shared memory, as a second backend option.** `--threads` builds the module with a shared memory and the atomics; without it, nothing changes. A page that is not cross-origin isolated loads the module without threads and runs on one core, with the same results.
2. **Parallel loops are split by ownership, not by locks.** A `parallel for` over the indices of arrays the loop borrows splits them into disjoint ranges, one per worker. The checker accepts it only if each turn writes nothing but the elements at its own index, the rule that already lets the [vector loops](../../../CHANGELOG.md) run 4 lanes at once, and calls nothing that allocates. The results are then those of the sequential loop, bit for bit, as for vector loops; tests compare both.
3. **One arena of the allocator per thread.** Each worker allocates from its own TLSF arena in the shared memory; only growing the memory takes a lock. A block released by another thread goes back to its arena through a list that thread empties.
4. **The loader keeps the pool.** It starts `navigator.hardwareConcurrency − 1` workers once, over the same memory, and hands out ranges; the main thread works too, and waits with `Atomics.waitAsync`.

## Not chosen now

- **Work stealing.** Equal ranges are enough for loops whose turns cost the same; stealing comes with a measure of a loop that needs it.
- **Moving values between threads with `sink`** ([decision 0003](0003-runtime-proved-at-compile-time.md)): after parallel loops, which cover the measured kernels.

## Status

Measured, not built. The ceiling is committed with its command; the backend comes after `Map` iteration.

## Consequences

- A module built with `--threads` needs a cross-origin isolated page for its threads; the Vite plugin will set the two headers in development and say so in production.
- Text crossing into the module still costs a copy: threads do not remove that one.
