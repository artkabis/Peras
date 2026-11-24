# 0043: Arenas, the temporaries of a turn released at once

_French translation: [../../fr/decisions/0043-arenas.md](../../fr/decisions/0043-arenas.md)._

_Status: protocol registered on 2026-11-24, before any prototype and any measure, as the rule of the project asks. It is the first step of phase 6 of the [roadmap](../reference/roadmap.md), "`Arena` and `Pool<T>` allocators", in the order the owner set on 2026-11-24: arenas, then `Pool<T>` (a decision of its own), then `@frame`, then the study of memory streamed by proximity, which needs both. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression; if none is, the decision says so and nothing lands._

## Context

What the compiler does today, read before writing this (finding M020):

- **Every heap block has one owner, and is released where its owner ends.** The lowering writes each release (`Stmt::Release`, `BufStmt::Release`), and the release verifier (`crates/perasc-ir/src/own.rs`) proves that every block is released exactly once on every path, and that each turn of a loop leaves the locals as it found them: a turn that allocates releases, or moves out. A temporary of a turn is therefore already released within that turn, one release per block.
- **Two allocators.** The debug allocator (decision 0001), by default on the command line and in the Vite dev server, poisons, keeps a quarantine and names the sites of every bug. TLSF (decision 0002), for production, takes a bounded time per operation; since decision 0016 it also keeps released blocks of 32 to 256 bytes, at most 64 per size, for the next allocation of their size: allocating and releasing such a block costs a few loads and stores. A larger block searches two bitmaps, splits, and merges with its neighbours when released.
- **Buffers are a header and their elements.** An array or a string is a 16-byte header that keeps its address; its elements follow it in the same block when they are few and hold no buffers, and are a block of their own otherwise, reallocated, at least doubled, when it grows (`crates/perasc-runtime/runtime/buffers.wat`). An array of strings owns its strings and releases them with itself (`DEEP`). Some of the runtime's own blocks, those of `split` for instance, are allocated with no site (site 0).
- **Allocation sites are numbered by the backend** (`crates/perasc-codegen-wasm`), for the messages of the debug allocator; the IR does not know them.
- **What the spec plans** ([spec](../spec.md#allocators)): "`Arena`: allocation by bumping a pointer, reset in a single operation. Ideal for a frame's temporary data", beside `Pool<T>(n)` and the general allocator. Nothing of it is built.
- **What decision 0032 measured**: a tree of shared nodes is 6.8 times slower than "an arena", three numbers in three arrays with indices. That arena is a layout of data, which `Pool<T>` will be about; this decision is about the cost of allocating and releasing temporaries.

So the question is not whether temporaries are released: they are, deterministically, by construction. It is whether releasing them **at once**, by moving a pointer back, is measurably cheaper than releasing them one by one through TLSF and its kept blocks, and at what cost in memory, bytes, and notions to learn.

What others do: Rust's `bumpalo` and C++'s `std::pmr::monotonic_buffer_resource` give an arena as a value, its allocations tied to its lifetime by the type system (Rust) or by the programmer (C++). Zig passes an allocator to everything that allocates, an arena among them. Game engines reset a frame allocator once per frame. Region inference (Tofte and Talpin, 1997, in the ML Kit) lets the compiler choose the region of every allocation, with no syntax; MLKit later added a collector, since inferred regions kept some programs' memory alive too long.

## Options

- **a0. As today**: TLSF and its kept blocks, every temporary released where its owner ends. The reference.
- **a1. An `arena { … }` block, written by the programmer.** Every block allocated inside it, by the program or by the runtime on its behalf, is taken by moving a pointer, and the whole of it is given back at the block's end, in one operation. A value made inside that would outlive the block, moved to a variable declared outside, returned, pushed into an array from outside, is an error at compile time, which shows where it leaves. A loop whose turns allocate is written `for (…) { arena { … } }`.
- **a2. Arenas chosen by the compiler, with no syntax.** The compiler takes from an arena every allocation it proves dead before the end of its innermost loop turn or function call, and gives the arena back at that turn's end or that call's return; a value it cannot prove so is allocated by TLSF, as today. Nothing to write and nothing to learn; nothing refused.
- **a3. The allocator alone, no arena**: TLSF keeps released blocks for more sizes than today (decision 0016's lists, widened), so that a temporary of any common size is allocated and released in a few loads and stores. No language change, no change of the modules.
- **An arena as a value**, `const a = Arena.new()` passed to what allocates, as `bumpalo` or Zig: not built. Peras has no lifetimes and stores no references (spec), so nothing can tie a value to the arena it came from without adding them, a change of the language far beyond this decision; it is named so that it is not taken for forgotten (finding M001).

In a1 and a2, arenas exist only in modules built with TLSF. A development build keeps the debug allocator, unchanged, with every check it makes today: an arena would hide the use after release it catches.

## Protocol, registered before any prototype

a1, a2 and a3 are prototyped behind `PERAS_PROTO_ARENA=a1|a2|a3`; without it, the compiler is as today, a0. Every module measured is built for production, `--allocator tlsf` (finding M029), by a release build of `perasc`.

### Before any measure: the definitions tested

The rule of a1 (what leaves a block) and the rule of a2 (what is proven dead in the turn or the call) are first written as an analysis that only reports, run on the workloads below and on every benchmark and example, which allocations each would take from an arena (finding M003). Two rules that disagree with this protocol, or a workload where an option would take nothing, are recorded, and the protocol amended before any measure, the amendment dated.

### The workloads

Owned modules on TLSF in Node, each option's module and a0's in one process, the order alternating round by round, 21 rounds, the median of the ratios to a0 and their range (finding M017):

- **W1, the dashboard of decision 0014** (`bench/data/dashboard.peras`, as written), on the data `scripts/bench-data.mjs` generates: a `split` per line, its fields parsed then dropped.
- **W2, temporaries of a frame** (`bench/arena/frame.peras`, written for this decision as a user would write it from JavaScript): 10,000 particles on a grid; each frame, for each particle, the array of its neighbours' indices is built, read, then dropped; 100 frames.
- **W3, a line of text a row** (`bench/arena/rows.peras`): 1,000,000 rows, each written as a template string of five fields, its bytes summed, then dropped.
- **W4, values that are kept** (`bench/arena/keep.peras`): 1,000,000 strings made and kept in an array, then summed: no arena can take them; it measures what an option costs where it cannot help.

### The four quantities

1. **Speed**: the time of each workload's work, its module loaded once, as above.
2. **CPU load**: the time a core is kept busy (`process.cpuUsage`), as decision 0012 measures it, over the same rounds.
3. **Memory and bytes**: the module's memory at the end of each workload (`memory.buffer.byteLength`, the peak, since memory never shrinks), and the size of each module, raw and gzip.
4. **Ease of writing**: W2 and W3 written as from JavaScript, then fixed until they build, for each option: the errors met, with their codes, the lines changed, the notions JavaScript does not ask for.

And, for every option, `node scripts/check-regression.mjs` against the commit of this protocol: every module of the benchmarks and examples identical or smaller, none larger; the compiler's median time ratio at most 1.05; its size reported.

### Correctness, before any figure counts

Each option passes `cargo test --workspace` with its variable set, the end-to-end tests of both allocators, and, for a1 and a2, the differential test with `PERAS_DIFFERENTIAL_SEEDS=2000`, extended with turns and calls whose values die or leave; then once with the option broken on purpose (an allocation that leaves its turn taken from the arena), to see a test fail (finding M015).

### Criteria, registered now

- An option that fails `check-regression`, or a correctness check above, is out.
- An option is an improvement only if its median ratio is at most 0.95 on at least one of W1 to W3, at most 1.02 on every workload, W4 included, its range taken into account, and its memory at most 1.10 times a0's on each workload.
- Among the improvements, the faster first, by the median of its ratios on W1 to W3; within 0.03 of each other, the one with fewer notions and fewer errors in (4).
- If no option is an improvement, a0 stays, the prototypes are removed, their figures kept here, and `Arena` leaves the roadmap until a workload shows it a cost to remove.

## Record

_To come: the definitions tested, the prototypes, the measure, the proposal._
