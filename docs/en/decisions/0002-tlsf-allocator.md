# 0002: The production allocator is TLSF

_French translation: [../../fr/decisions/0002-tlsf-allocator.md](../../fr/decisions/0002-tlsf-allocator.md)._

## Context

[Decision 0001](0001-webassembly-backend.md) gave Peras its own allocator, linked only into modules that use the heap. Until now, only the debug allocator existed. It is slow on purpose:

- it poisons released memory, which costs time on every release;
- it keeps freed blocks in quarantine, so reuse comes late;
- it checks a red zone at every release;
- it searches a size class's free list block by block.

A production module needs allocation in bounded time, memory reused at once, and fragmentation kept low.

## Decision

1. **Production modules use TLSF**, Two-Level Segregated Fit (Masmano, Ripoll, Crespo and Real, 2004). It is written in WebAssembly text (`crates/perasc-runtime/runtime/alloc_tlsf.wat`), like the debug allocator, and assembled when `perasc` builds.
2. **Development keeps the debug allocator.**
   - On the command line, `perasc build` uses `--allocator debug` by default; production builds pass `--allocator tlsf`.
   - In the Vite plugin, the dev server links the debug allocator and `vite build` links TLSF. The plugin's `allocator` option overrides that choice.
3. **The two allocators have the same exports** and share the buffer functions, written once (`runtime/buffers.wat`). Generated code does not depend on which one is linked.

## How it works

- **Size classes.** Free blocks are kept in 384 lists by size: 24 first levels, one per power of two, each split into 16 classes, with one class per 16 bytes below 256.
- **Finding a block.** Two bitmaps say which lists are not empty. A block that fits is found with `i32.clz` and two `i32.ctz`, without walking any list.
- **Splitting.** A block larger than needed is split, and what is left stays free.
- **Merging.** A released block merges at once with its free neighbours, so two free blocks are never side by side. A free block that ends the heap gives its memory back to it.
- **Bounded time.** Every operation takes bounded time, whatever the size asked for and the state of the heap.
- **Checks.** It catches only what costs nothing: releasing something that is not one of its blocks, or releasing a block twice. It keeps no allocation site per block, so its leak report gives the number of blocks alive, not where they come from. The debug allocator covers the rest, during development.

## Measurements

The program `churn` builds an array of 20 strings 100,000 times, about 6 million allocations and as many releases, under Node 26:

| Allocator | Time    | Blocks alive at the end |
| --------- | ------- | ----------------------- |
| debug     | 1190 ms | 0                       |
| TLSF      | 444 ms  | 0                       |

That is 2.7 times faster. Functions that spend their time at the JavaScript boundary gain less: there, encoding and decoding strings dominates.

The tests of `crates/perasc-runtime/tests/tlsf_alloc.rs` check:

- reuse: 10,000 allocations and releases leave the end of the heap where it was;
- merging both ways, and splitting;
- invalid and double releases;
- memory that runs out cleanly;
- 6,000 random operations against a model. Every 50 steps, the heap's whole layout is checked: contiguous blocks, back links, no two free neighbours. The heap never exceeds twice its peak of live bytes, plus 128 KiB. Once everything is released, it is empty again.

## Consequences

- Fragmentation is bounded, not zero. A block wastes at most a fraction of its size class: under 1/16 above 256 bytes, and under 16 bytes below. Recovering memory left between live blocks will need compaction: elements can move behind their array header, without changing the header's address.
- The end-to-end tests run modules on both allocators.
