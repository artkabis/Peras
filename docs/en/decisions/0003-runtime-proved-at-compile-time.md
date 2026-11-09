# 0003: A runtime proved at compile time, not a micro-OS

_French translation: [../../fr/decisions/0003-runtime-proved-at-compile-time.md](../../fr/decisions/0003-runtime-proved-at-compile-time.md)._

## Context

The [technical architecture](../architecture.md) proposes running owned modules as a "virtual micro-OS" inside WebAssembly memory:

- a software MMU with 4 KB pages and 32-bit handles;
- an allocator made of slabs and a buddy allocator;
- guard pages;
- a scheduler of fibers;
- copy on write between threads.

The goal is right: memory managed deterministically, without a garbage collector, by the module itself.

The mechanisms come from a different situation. An operating system knows nothing of the programs it runs, so it must translate, intercept and check at run time. Peras is the opposite case. Before any code runs, its compiler has proved:

- who owns each value;
- that no value is aliased while it changes (`inout` is exclusive);
- that no reference is stored inside data;
- where each buffer is released, exactly once on every path.

Mechanisms that check at run time would pay, on every memory access, for what is already known.

## Decision

1. **Peras is a runtime proved at compile time.** The documentation and the presentations of the project describe it so, not as a micro-OS.
2. **What stays from the proposal, as it was proposed:**
   - the production allocator, TLSF with `i32.clz` ([decision 0002](0002-tlsf-allocator.md));
   - arrays as columns (already done), SIMD (phase 6) and loop fusion;
   - the `@frame` rule, a proof that rendering code does not allocate: the form the "zero GC linter" takes;
   - lock-free ring buffers on `Atomics`, between workers;
   - 256-byte alignment of buffers shared with WebGPU (phase 6);
   - a memory inspector: a map of the heap and the history of each value, built on what the debug allocator already records.
3. **What stays in another form:**
   - **Compaction instead of a page table.** Every array has a 16-byte header that holds the address of its elements, and only one owner holds the header. A compactor can move elements and update one word, with no change to any address a program holds, and no translation on each access.
   - **`sink` between threads instead of copy on write.** A value passed `sink` changes owner, so sending it to a worker is a move without a copy. The compiler proves the sender no longer touches it, which no page protection could do.
   - **Software checks instead of guard pages.** WebAssembly memory has no page protection. Every index is already checked, and the debug allocator puts a red zone after each block. A faulty module restarts at the level of its Web Worker.
   - **Field reordering** only once records have a layout in memory (pixels, GPU buffers), with an attribute that keeps the written order for interoperability.
   - **Space-filling curves** as a library type chosen on purpose, such as a `Grid2D` in Morton order, not as an automatic transformation.
4. **What does not stay:**
   - fibers, until WebAssembly can switch stacks (JSPI or the stack-switching proposal); Web Workers give parallelism now;
   - energy counters (RAPL) in the browser, which exposes none; energy is measured on native benches;
   - forbidding `push`, which never involves the browser's garbage collector;
   - figures announced before they are measured.

## Consequences

The roadmap keeps its order. Three system projects are added to it:

1. **Heap compaction** behind array headers, after the standard library: the follow-up to TLSF.
2. **Concurrency**: Web Workers, values moved to them with `sink`, ring buffers. It needs managed mode (phase 4) for the browser side; its own written decision will set the boundary.
3. **The memory inspector**, with the editor extension in phase 7.

Every claim of performance comes with its measurement, as decision 0002 does for TLSF.
