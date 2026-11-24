# 0043: Arenas, the temporaries of a turn released at once

_French translation: [../../fr/decisions/0043-arenas.md](../../fr/decisions/0043-arenas.md)._

_Status: protocol registered on 2026-11-24, before any prototype and any measure, as the rule of the project asks; options measured at `d59e034`; proposal: a0 stays, for the owner to decide; option a1b added on the owner's request, registered before its prototype. It is the first step of phase 6 of the [roadmap](../reference/roadmap.md), "`Arena` and `Pool<T>` allocators", in the order the owner set on 2026-11-24: arenas, then `Pool<T>` (a decision of its own), then `@frame`, then the study of memory streamed by proximity, which needs both. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression; if none is, the decision says so and nothing lands._

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

### The definitions tested, and two amendments (2026-11-24, before any measure)

The rule of a2 is written as an analysis that only reports (`crates/perasc-ir/src/arena.rs`): it groups the `owned` locals whose blocks are one value's (a local moved into another, a buffer pushed into an array of buffers, a key moved into a map), keeps a group whose blocks are allocated in the function by the runtime and none leaves, and gives it the innermost loop turn, or the call, that holds every mention of it. A statement that allocates or grows such a group runs in the arena, unless it calls a function, grows a block that is not taken, or sits inside a loop, below the group's home, that gives its room back each turn: then TLSF serves it, as today, and since every release is kept, a block may come from either. a1's rule is the same, its region the block written.

Run on the 201 Peras files of the benchmarks, the examples and `bench/arena`, it would take 603 of their 886 allocations. On the workloads:

| Workload            | Allocations taken                                                                | Where                                                                                   |
| ------------------- | -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| W1, the dashboard   | `isCountry` 1 of 1, `parse` 1 of 3 (the fields of `split`), `summarize` 19 of 34 | the call, and a turn of its loop over lines                                             |
| W2, the frame       | 5 of 5                                                                           | the call (the positions), a frame's turn (the grid), a particle's turn (its neighbours) |
| W3, the rows        | 2 of 2                                                                           | a row's turn                                                                            |
| W4, as registered   | 2 of 2                                                                           | the call                                                                                |
| W4, amended (below) | 0 of 2                                                                           |                                                                                         |

Two amendments follow, made before any measure:

1. **W4 measured nothing it was meant to.** Its strings are kept in an array until the function returns, then dropped: they die within the call, and a2 takes them. W4 is now two functions, `make(n)` returning the array of strings and `run(n)` summing what `make` gives: a value returned leaves its call, and nothing is taken. The registered W4 is kept as W4b, a value kept for a whole call, which a2 takes.
2. **W3 cannot sum a row's bytes**: Peras has no `charCodeAt` yet (finding F309, `E0215` on `byteAt`). It sums each row's length and the place of its first comma instead, which reads the row without allocating.

### The prototypes (2026-11-24)

- **a2** (`crates/perasc-ir/src/arena.rs`, `PERAS_PROTO_ARENA=a2`): the analysis above, then a rewrite of the IR: a mark where each home begins, the arena's room given back at the end of each turn of a home that is a loop, after the loop, and at each return; `arena_flag(1)` and `arena_flag(0)` around each statement the arena takes. TLSF with an arena (`alloc_tlsf.wat`, between `;; @proto a2` and `;; @endproto`): one block of 256 KiB taken from TLSF the first time, allocation by moving a pointer while the flag is on, a release inside it doing nothing. It is linked only into a module that uses an arena.
- **a1** (`PERAS_PROTO_ARENA=a1`): `arena` before a block, parsed only under the variable; the lowering puts the block between two marks; the same pass, with the blocks written as the only homes; a value made in one that outlives it is refused at the block. **Departure**: the prototype refuses it with `E0900`, whose message does not name the value; a code of its own would come with its landing.
- **a3** (`PERAS_PROTO_ARENA=a3`): blocks of 257 bytes to 4 KiB rounded up to 256 and kept, as decision 0016 keeps the small ones, in 15 more lists.

Each passes `cargo test --workspace` with its variable set, and the differential test with `PERAS_DIFFERENTIAL_SEEDS=2000`. The differential test first missed the arena: it linked every module to TLSF as it is, so the arena's calls did nothing. It now links the prototype's runtime, and it gained a grid grown inside a loop whose turns have values of their own, written in two nested arenas under a1. Broken on purpose (a statement under a loop that gives its room back taken all the same), a2 and a1 each fail it (finding M015).

### The measure (2026-11-24)

At `d59e034`, `node scripts/bench-arena.mjs`, 21 rounds, every option in one process, the order alternating ([results](../../../bench/arena/RESULTS.md)); then `node scripts/check-regression.mjs 1ff4de1 PERAS_PROTO_ARENA=…`, the commit of the protocol, for each option. Machine: Intel Xeon @ 2.10 GHz, 4 cores; Node 24.22.1.

The time of each workload, the median of its ratios to a0 round by round:

|     | W1    | W2, the fireflies | W3        | W4    | W4b   |
| --- | ----- | ----------------- | --------- | ----- | ----- |
| a1  | 1.004 | **0.937**         | **0.940** | 1.008 | 1.011 |
| a2  | 0.971 | 1.163             | **0.935** | 1.001 | 0.983 |
| a3  | 1.046 | 1.017             | 1.035     | 1.019 | 1.027 |

The CPU load follows the time: CPU over wall is 0.99 to 1.01 everywhere, one thread.

Memory at the end of each workload, against a0's: a1 and a2 add the arena's 256 KiB where it is used: W3 goes from 64 KiB to 320 KiB, five times; W2 from 832 KiB to 768 KiB under a1 (less), 960 KiB under a2; W1 within 1 %. a3 adds up to 8 % (W2).

Bytes and the compiler, by `check-regression`:

|     | modules larger                                          | compile time, median ratio | module of W3                      |
| --- | ------------------------------------------------------- | -------------------------- | --------------------------------- |
| a1  | 0 of 250 (no program of the repository writes an arena) | 1.040                      | 4,434 to 4,774 bytes when it does |
| a2  | 61 of 250                                               | 0.998                      | 4,778 bytes                       |
| a3  | 91 of 250                                               | 1.027                      | 4,495 bytes                       |

Ease of writing, W2 and W3 written as from JavaScript: today's language meets 10 `E0210` on W2 (an `i32` in a float, three rounds of fixes) and one `E0215` on W3 (`byteAt`, F309), the same under every option; a1 asks for one more notion, a block and what may leave it, and two lines per program, and met no error of its own.

Against the criteria registered:

- **a2 is out**: 1.163 on W2, above 1.02; and 61 modules larger.
- **a3 is out**: 1.046 on W1; no workload at 0.95; and 91 modules larger.
- **a1 is out**: it is the only option with a gain, 0.937 on W2 and 0.940 on W3, and it makes no module of the repository larger, but its memory on W3 is five times a0's, above 1.10. Its arena's 256 KiB are a fixed cost that a small program pays whole.

### Proposal

**a0 stays**: no option is an improvement by the criteria registered before the measure. As the protocol says, the prototypes are removed, their figures kept here and their code in git at `d59e034`, and `Arena` leaves the roadmap until a workload shows a cost to remove.

What the measure teaches, for a later decision rather than this one: the gain of a written arena is real, about 6 % on temporaries, and what made it fail is the size of its block, not its rule; an arena that starts small and grows, or one sized from the program's own allocations, would be another option, measured against the same criteria. a2's flags around every statement cost it more than the arena saves where the statements are many and small (W2). The owner decides.

### Amendment: option a1b (2026-11-24, after the measure of a1, a2 and a3, before any prototype or measure of a1b)

The owner asked whether a1 could be made to pass. a1b is defined here from what the measure of a1 showed, which is why it is registered as an option of its own, after that measure and before its own: its definition is fixed now, and **the criteria are the ones registered above, unchanged**.

- **a1b** is a1, its syntax, its rule and its error, with three changes, each answering a cost measured:
  1. **An arena that starts small and grows** (memory, W3 five times a0's under a1): its first block is 4 KiB; an allocation that does not fit is served by TLSF, and marks the arena as too small; the next time the arena is marked while empty, it gives its block back to TLSF and takes one twice as large, up to 1 MiB.
  2. **The arena on for a whole block** (bytes and time of `arena_flag` around each statement, a2's loss on W2): on from the block's start to its end, turned off only around a statement or a condition that calls a function, makes a function value, grows a block not taken, or sits below a nested arena that gives its room back; off before every `break`, `continue` and `return`.
  3. **The analysis run only on the functions that hold a block written** (the compiler's time, 1.040 under a1).
- It is prototyped behind `PERAS_PROTO_ARENA=a1b`, and passes the same checks of correctness as the others: `cargo test --workspace`, the differential test with `PERAS_DIFFERENTIAL_SEEDS=2000` and its nested arenas, and once broken on purpose to see it fail.
- It is measured as the others were: `scripts/bench-arena.mjs`, 21 rounds, in one process with a0, a1, a2 and a3, its versions of W2 and W3 those of a1 (`bench/arena/a1`); then `check-regression` against the commit of the protocol.
