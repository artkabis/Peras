# Technical architecture: system choices and their value

_French translation: [../fr/architecture.md](../fr/architecture.md)._

This reference document justifies the software and hardware engineering decisions of the Peras project. Developers use it as a specification, technical auditors as a validation grid, and investors and public bodies as a statement of the technological break it aims for.

Every choice has a **status**:

- <span class="status compiled">done</span>: in the compiler today, tested;
- <span class="status checked">in progress</span>: part of it exists;
- <span class="status planned">planned</span>: in the [roadmap](reference/roadmap.md) or the [spec](spec.md), not in the language yet;
- **to decide**: proposed here, not on the roadmap yet; a written decision, like [0001](decisions/0001-webassembly-backend.md), will settle it.

The **engineering notes** say what the web platform actually allows, so that every promise made to a customer or a funder can be kept. Figures are goals, to be measured with `bench/backends` before they are announced as results.

## I. Compilation and static analysis (`perasc`)

### 1. A compiler in safe Rust

**Technical detail.** The whole pipeline is written in Rust without a single `unsafe` block: lexing, parsing, type and ownership checks, the intermediate representation and WebAssembly generation.

**Value.** The tool itself cannot corrupt memory. `perasc` runs at native speed and can be compiled to WebAssembly, to run in a browser or on edge servers.

**Status.** <span class="status compiled">done</span>: `unsafe_code = "forbid"` for the whole workspace, with no warning allowed. The compiler running in the browser is <span class="status planned">planned</span>, as the documentation playground of phase 7.

### 2. Two targets: `.peras` and `.apeiron`

**Technical detail.** From one syntax inspired by TypeScript, the compiler produces either WebAssembly (`.peras`) or JavaScript (`.apeiron`).

**Value.** The target is chosen by the kind of work, without rewriting business code.

**Status.** `.peras` <span class="status compiled">done</span>. `.apeiron` <span class="status planned">planned</span>, phase 4.

**Engineering note.** The [spec](spec.md#managed-mode-apeiron) makes managed mode the target for the DOM, `async` and browser APIs, in ordinary JavaScript. JavaScript that "only indexes typed arrays" would be another mode: **to decide**. Both can coexist, the second as a fallback for `.peras` where WebAssembly is unavailable.

### 3. Affine types and ownership checks (`perasc-check`)

**Technical detail.** At compile time, each value has one owner. It moves on assignment, on `return` and with `sink`, and is borrowed otherwise. A use after a move blocks the build ([E0204](errors/E0204.md)), as do two arguments reaching one variable that one of them changes ([E0207](errors/E0207.md)).

**Value.** No use after free, no double free and no leak, with neither a garbage collector nor reference counting at run time.

**Status.** <span class="status compiled">done</span>. Lowering inserts every release, and a release verifier proves that every owned value is released exactly once on every path.

**Engineering note.** Peras follows mutable value semantics (Swift, Hylo), not Rust's references. No reference is stored inside data, so there is **no lifetime to write, nor to infer**: developers only write `inout`, `sink` and `&` at the call. An "80 % inference rate" therefore measures nothing. What can be measured is the share of programs that compile with no annotation at all.

### 4. Field reordering and automatic padding

**Technical detail.** The compiler sorts fields from widest to narrowest (`v128`, `f64`, `u32`, `u8`) and aligns each one with $A_{align} = (A_{raw} + (N - 1)) \ \& \ \sim(N - 1)$, inserting the padding needed.

**Value.** No penalty for unaligned access, and more compact records.

**Status.** **To decide.**

**Engineering note.**

- Today, no structure has a memory layout. A structure is replaced by its scalars (locals, multiple results), and an array of structures is stored as columns (see 5): there is no padding and no unaligned access.
- Reordering will matter once records are laid out in memory: the pixels of a `Framebuffer`, buffers shared with WebGPU, `sizeOf<T>()`.
- The spec then promises a **visible, fixed layout**. Interoperability will need an attribute that keeps the written order, as `#[repr(C)]` does in Rust.
- The memory saved compared with V8 objects is to be measured, not announced.

### 5. Structure of arrays and SIMD

**Technical detail.** Arrays of objects are stored as contiguous columns, one per property. Loops over those columns compile to WebAssembly `v128` vector instructions.

**Value.** Cache lines full of useful data, and 2 to 16 values per instruction depending on their size.

**Status.**

- Columns <span class="status compiled">done</span>: `Array<Point>` keeps one buffer per field, so `points[i].x` and a loop over one field touch one buffer.
- SIMD <span class="status planned">planned</span>, phase 6.

**Engineering note.** WebAssembly SIMD registers are 128 bits wide: 4 `f32`, 2 `f64` or 16 `u8` per instruction. A 4x to 8x speedup is reachable for kernels on `f32` or narrow integers, not in general. WebAssembly has no 256-bit vectors.

### 6. Loop fusion

**Technical detail.** Chained operations on a collection (`.map().filter().reduce()`) are combined into one loop.

**Value.** Fewer memory reads and writes, and no intermediate array.

**Status.** <span class="status planned">planned</span>: `map`, `filter` and `reduce` come with the standard library (phase 3). Fusion will be an IR pass, or come from compiling these methods as lazy iterators.

### 7. Space-filling curves for spatial indexing

**Technical detail.** The indices of 2D and 3D grids are interleaved bit by bit (Morton or Z-order), so that cells close in space are close in memory.

**Value.** Many more cache hits when walking neighbourhoods.

**Status.** **To decide.**

**Engineering note.**

- Morton and Hilbert are two different curves. Morton costs a few instructions of bit interleaving. Hilbert keeps better locality, but costs more to compute.
- Changing the order of indices changes what `grid[i]` means, and makes a row-by-row walk slower. This will be a library type chosen on purpose, such as a `Grid2D` in Morton order, not an automatic transformation.
- Cache hit rates near 100 % depend on the walk.

## II. Virtual kernel and memory management (`peras-kernel`)

### 8. Software MMU and 4 KB pages

**Technical detail.** Memory is split into 4 KB virtual pages. Code handles 32-bit handles (20 bits of page, 12 bits of offset), translated in constant time by a page table.

**Value.** Blocks can be moved and compacted in the background without invalidating the developer's references.

**Status.** **To decide.**

**Engineering note.**

- The translation adds a table read and some arithmetic to **every** memory access, hot loops included.
- Peras has a better handle on the problem. No reference is stored in data, so each buffer has exactly one owner, the only place to update when it moves.
- Arrays already have a 16-byte header that holds the address of their elements: the elements can move while the header stays. A compactor can move data without a page table, with a single indirection that the IR knows how to hoist out of loops.
- Compacting during browser idle time is possible between two exported calls, when no buffer is lent.

### 9. A deterministic allocator

**Technical detail.**

- Slabs for small objects of fixed size.
- A TLSF index (_Two-Level Segregated Fit_) that finds a free block with `i32.clz`.
- A buddy allocator (power-of-two blocks) for large arrays.

**Value.** Allocation and release in bounded time, with controlled fragmentation.

**Status.** TLSF <span class="status compiled">done</span> ([decision 0002](decisions/0002-tlsf-allocator.md)): bounded time, blocks merged at once, linked by `vite build` and `--allocator tlsf`. The debug allocator stays for development: poisoned memory, red zones, a quarantine and leak counts, and every error names where the block was allocated and released.

**Engineering note.**

- TLSF alone serves variable sizes in O(1). Combining it with a buddy allocator is redundant: pick one per size class.
- No allocator guarantees **0 % fragmentation**:
  - a buddy allocator rounds each block up to a power of two, wasting up to almost half of it inside;
  - TLSF bounds fragmentation without removing it;
  - only slabs have no external fragmentation, and only within a slab.
- The promise that holds: bounded time, bounded fragmentation, and compaction to recover it (see 8).

### 10. Guard pages

**Technical detail.** Inaccessible pages between memory regions catch overflows.

**Value.** An overflow stops before it corrupts neighbouring data.

**Status.** Software equivalent <span class="status compiled">done</span>.

**Engineering note.**

- WebAssembly linear memory has **no page protection**: an access within its bounds is never caught by hardware. A guard page can only be a software check.
- That is what Peras already does. Every index is checked before any access (`RangeError`), and in development, every block has a 16-byte red zone checked when it is released.
- The language has no pointer arithmetic, so an overflow can only come from a compiler bug. The release verifier and fuzzing hunt for those.
- Restarting a failed module is possible at the level of a Web Worker.

### 11. Cooperative scheduler (fibers)

**Technical detail.** Lightweight fibers in the runtime split long tasks into time slices.

**Value.** A smooth interface that never blocks the browser's main thread.

**Status.** **To decide.**

**Engineering note.** WebAssembly cannot switch stacks yet. Three routes exist:

- JSPI (_JavaScript Promise Integration_), being standardized;
- the stack-switching proposal;
- a compiler transformation of functions into state machines, as Rust does for `async`. It works everywhere, but can only pause at marked points.

For heavy computation, Web Workers give real parallelism without fibers.

## III. Interoperability, network and hardware

### 12. Transfer between threads without copying

**Technical detail.** Read-only pages shared in a `SharedArrayBuffer`, duplicated only on the first write (copy on write).

**Value.** `postMessage` serialization, in O(n), replaced by a transfer in O(1).

**Status.** **To decide.**

**Engineering note.**

- Neither JavaScript nor WebAssembly can detect a write to a page: copy on write would need a compiled check before every write.
- Peras's model offers better:
  - a value passed `sink` changes owner, so sending it to a worker is a **move without a copy**, safe because the sender can no longer read it;
  - a value that is only read can be shared without a copy, since nobody writes it.
- Transferable `ArrayBuffer`s already make `postMessage` O(1).
- A `SharedArrayBuffer` requires cross-origin isolation (COOP and COEP headers), a deployment constraint to document.

### 13. Lock-free ring buffers

**Technical detail.** FIFO queues of power-of-two size, indexed with the mask `& (n - 1)` instead of `%`, and synchronized by `Atomics.load` and `Atomics.store`.

**Value.** High-frequency network and audio streams, with no allocation and no lock.

**Status.** **To decide**, feasible as described.

**Engineering note.**

- This is the standard single-producer, single-consumer scheme, the one `AudioWorklet` uses.
- Several producers need a compare-and-swap (`Atomics.compareExchange`).
- `Atomics` works on integer typed arrays (`Int32Array`), not on the `ArrayBuffer` itself.

### 14. A bridge to WebGPU

**Technical detail.** Buffers aligned on 256 bytes, the offset alignment WebGPU requires for uniform and storage buffers.

**Value.** Data ready for the GPU, without repacking.

**Status.** <span class="status planned">planned</span>, phase 6.

**Engineering note.** WebGPU reads neither JavaScript nor WebAssembly memory directly. Data goes through `writeBuffer` or a mapped buffer, so there is **at least one copy**, even on chips with unified memory, where the browser keeps control. Aligning on 256 bytes removes the repacking, not the copy. The promise that holds: one copy, with no conversion.

## IV. Tooling, safety and telemetry

### 15. An invariant linter

**Technical detail.** A build-time analysis refuses patterns that allocate, and shared mutation that is not atomic.

**Value.** Proof, before deployment, that part of the code does not allocate.

**Status.** <span class="status planned">planned</span>, as the spec's `@frame` rule: a `@frame` function may neither allocate nor call a function that allocates, and the compiler proves it or refuses the build.

**Engineering note.**

- A `.peras` module holds no JavaScript object: `push` grows a buffer in WebAssembly memory and never involves the browser's garbage collector. Forbidding `push` everywhere would remove arrays from the language.
- The right rule targets `@frame` functions, and the JavaScript glue, which today creates objects at the boundary.
- Shared mutation that is not atomic is already impossible: `inout` is exclusive ([E0207](errors/E0207.md)).
- ISO 27001 and DORA certify how a company is organized, not a program. These proofs can feed its audits; they do not replace them.

### 16. Memory inspector and energy profiling

**Technical detail.**

- A heat map of memory.
- A timeline that replays the life of values: allocation, loan, move, release.
- Readings of the processor's energy counters (RAPL).

**Value.** Much faster low-level debugging, and energy-saving reports.

**Status.** Inspector **to decide**. Its data already exists: the debug allocator knows every live block and where it was allocated, and E0204 tells the history of a value.

**Engineering note.** No browser exposes RAPL. Even natively, on Linux, its counters need elevated rights and measure the whole processor, not a tab. Energy profiling is therefore possible on a native or server test bench, not on the user's machine. Sustainability reports will rely on those benches, with their method published.

## Summary

| Choice                   | Key mechanism                                | Hardware impact                  | Economic value            | Status                                      |
| ------------------------ | -------------------------------------------- | -------------------------------- | ------------------------- | ------------------------------------------- |
| Checked ownership        | Affine types, releases inserted and verified | No garbage collector             | No pauses, no leaks       | <span class="status compiled">done</span>   |
| Arrays as columns        | One buffer per field                         | Caches full of useful data       | Faster computation        | <span class="status compiled">done</span>   |
| SIMD                     | `v128` instructions                          | 2 to 16 values per instruction   | Faster computation        | <span class="status planned">planned</span> |
| Deterministic allocator  | TLSF, `i32.clz`                              | Bounded time and fragmentation   | Stable heavy applications | <span class="status compiled">done</span>   |
| Compaction               | Elements moved behind their header           | Memory recovered while running   | No saturation over time   | **to decide**                               |
| Transfer between threads | `sink`: a move without a copy                | O(1) instead of O(n)             | Background processing     | **to decide**                               |
| WebGPU alignment         | Buffers aligned on 256 bytes                 | One copy, no conversion          | Local 3D and AI           | <span class="status planned">planned</span> |
| `@frame`                 | Proof of no allocation                       | No memory growth while rendering | Steady frames             | <span class="status planned">planned</span> |
| Energy profiling         | RAPL on a native bench                       | Reproducible measurement         | GreenTech reports         | **to decide**                               |

What these choices cost, and in which order they come: the [roadmap](reference/roadmap.md). Every choice still **to decide** will go through a written decision, with measurements, like [0001](decisions/0001-webassembly-backend.md).
