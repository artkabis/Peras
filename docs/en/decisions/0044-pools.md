# 0044: Pools, many values of one type held by handles

_French translation: [../../fr/decisions/0044-pools.md](../../fr/decisions/0044-pools.md)._

_Status: protocol registered on 2026-11-24, before any prototype and any measure, as the rule of the project asks; workloads amended twice before any measure; options measured at `5194fdb`; proposal: p0 stays; amended on 2026-11-25 with p4, before its prototype; p4 measured at `49c5842`; proposal: p0 stays; the owner kept p0; landed. It is the step of phase 6 of the [roadmap](../reference/roadmap.md) that follows the arenas ([decision 0043](0043-arenas.md)), "the `Pool<T>` allocator", and comes before `@frame` and the study of memory streamed by proximity, which needs a fixed budget of slots. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression; if none is, the decision says so and nothing lands._

## Context

What the compiler does today, read and tried before writing this (finding M020):

- **An array of structures is stored as columns**, one buffer per field: `Array<Spark>` keeps its `x`, its `y` and its `parent` each in a buffer of its own, and a loop over one field touches one buffer.
- **A program with many values that come and go, and point to one another, writes indices by hand**: arrays, and a number for each link (`bench/shared/workloads/w2-index.peras`). Decision 0032 measured it 6.8 times faster than a tree of `Rc`, with 4.75 times less memory. What it must write itself: the list of free slots, to reuse one; and nothing catches an index kept after its slot was freed and reused, which then reads another value, in silence.
- **Generic structures and functions compile**: `struct Slots<T> { values: Array<T>; live: Array<bool>; }`, filled by `fn put<T>(s: inout Slots<T>, v: sink T)`, builds and runs (tried on 2026-11-24 at `658321d`); a value moved into a generic function is declared `sink`, or the checker refuses it (`E0209`).
- **The standard library is not written in Peras**: its methods are built into the checker and the runtime (`crates/perasc-check/src/stdlib.rs`); there is no Peras source the compiler ships and every program imports.
- **What comes next needs a budget**: `@frame` forbids allocation while rendering, so its values live in slots reserved beforehand; the proximity study keeps a fixed number of chunks and evicts by a policy.

What others do: generational indices (Rust's `slotmap`, the entity systems of game engines): a handle is a slot and a generation; freeing a slot bumps its generation, and an old handle is refused. Object pools of fixed capacity in game engines and embedded systems. C++'s `std::pmr::unsynchronized_pool_resource`.

## Options

- **p0. As today**: arrays and indices, the free list written by hand. The reference.
- **p1. `Pool<T>` built into the compiler, generational handles**: `Pool.new()`, `pool.insert(v)` gives a `Handle<T>`, a slot and a generation; `pool.get(h)` an `Option<T>`, `None` for a handle whose value was removed; `pool.set(h, v)`, `pool.remove(h)`, `pool.size`, `for (const [h, v] of pool)`. Each access compares the generation.
- **p2. The same, with plain handles**: a handle is a slot; a handle kept after its value was removed reads whatever the slot holds now.
- **p3. `Pool<T>` written in Peras**, a generic structure over arrays (values, generations, free slots) with the same methods as p1, as a source the compiler ships and a program imports (`import { Pool } from "peras:pool"`), no new type in the checker. It needs that way of shipping a Peras source, which does not exist: it is built for this decision, and its cost counted.
- **A capacity fixed at creation**, `Pool.withCapacity(n)`, `insert` giving `None` when full and nothing growing, is what `@frame` will need; it is measured as a variant of the option chosen, W3 below, not as an option of its own.

## Protocol, registered before any prototype

p1, p2 and p3 are prototyped behind `PERAS_PROTO_POOL=p1|p2|p3`; without it, the compiler is as today, p0. Every module measured is built for production, `--allocator tlsf` (finding M029), by a release build of `perasc`.

### Before any measure: the definitions tested

Each workload is first written for p0 and checked to give what its managed module gives; the operations each option offers are checked against what the workloads use (finding M003). A workload an option cannot express, or a definition found wrong, is recorded and the protocol amended before any measure, the amendment dated.

### The workloads

Owned modules on TLSF in Node, each option's module and p0's in one process, the order alternating round by round, 21 rounds, the median of the ratios to p0 and their range (finding M017). Each workload is also called twice on one instance, and its second answer checked (finding M035).

- **W1, the sparks** (`bench/pool/sparks.peras`): each frame of 600, a thousand sparks are born, each holding the handle of the spark it came from, and those older than their life die; the live ones move, and read their parent's place through its handle when it still lives. The scene of the measure: the same sky for each option.
- **W2, a scene graph with removals** (`bench/pool/scene.peras`): a tree of 100,000 nodes, each holding its parent's and its children's handles; a third removed, then as many inserted, then the depth of each summed.
- **W3, a budget of chunks** (`bench/pool/chunks.peras`): 256 slots of 4 KiB each, chunks asked for along a path of 100,000 steps and the least recently used evicted when the budget is full: the base of the proximity study. Measured for each option, and with a capacity fixed at creation under the option chosen.
- **W4, no pool** (`bench/pool/plain.peras`): a program of arrays that uses none; with every benchmark and example, `check-regression` holds it identical.

### The quantities

1. **Speed**: the time of each workload, as above.
2. **CPU load**: the time a core is kept busy, `process.cpuUsage`, over the same rounds.
3. **Memory and bytes**: the module's memory at the end of each workload, and each module's size, raw and gzip; `check-regression` against the commit of this protocol, and the compiler's size.
4. **Ease of writing**: W1, W2 and W3 written for each option as a user would from JavaScript, then fixed until they build: the errors met, with their codes, the lines, the notions JavaScript does not ask for.
5. **Safety**: `bench/pool/stale.peras`, a program that keeps a handle after removing its value, then reads through it: under each option, whether the stale read is caught (`None`, or a fault) or gives another value in silence.

### Correctness, before any figure counts

Each option passes `cargo test --workspace` with its variable set, the end-to-end tests, and the differential test with `PERAS_DIFFERENTIAL_SEEDS=2000`, its generator extended with pools whose values are inserted, removed and read through handles kept across removals; then once broken on purpose (p1 and p3: the generation not compared), to see a test fail (findings M015, M034).

### Criteria, registered now

Memory is bounded as a ratio and in bytes, and a module identical to p0's counts at 1.000 (finding M037).

- An option that fails `check-regression` for a program that does not use it, or a check of correctness above, is out.
- An option is an improvement only if it is **safer than p0** (point 5: the stale read caught), its time at most **1.05** times p0's on every workload, its range taken into account, its memory at most **1.10 times p0's or one page of 64 KiB more**, and W1 to W3 written in **no more lines** than p0, with no more errors met.
- Among the improvements, the one with fewer notions and fewer lines, then the faster by the median of its ratios on W1 to W3.
- If no option is one, p0 stays, the prototypes are removed, their figures kept here, and the roadmap says why.

## Record

### The definitions tested, and an amendment (2026-11-24, before any prototype)

The four workloads and the program of point 5 are written for p0 (`bench/pool/`), as a user would from JavaScript: W1 met six `E0210` (an integer divided as a float, `pop()` giving an `Option<i32>`, an `i32` added to a float), and every workload stopped on an overflow in a development build, the random numbers `seed * 1103515245 + 12345` overflowing on purpose, fixed with `wrappingMul` and `wrappingAdd`; W2, W3 and the program of point 5 were written after W1 and met no error. Built for production, each gives the same twice on one instance, the same as built for development, and the same as its managed module (W1 433595.05078125, W2 487013, W3 12797022). Under p0, the program of point 5 reads 99, the value given to the slot after the one kept was removed: in silence, as expected.

One definition is amended: W1 moves each spark in place (`sparks[i].x += sparks[i].vx`), and the operations registered for p1 and p2 offer only `set(h, v)`, a whole value. **p1 and p2 also index by a handle, `pool[h]`, to read and write a field**: under p1 a handle whose value was removed stops the program, as an index out of bounds does, `get(h)` staying the read that gives `None`; under p2 it reads the slot as it is. **p3**, a structure written in Peras, cannot define `[]`: it offers `pool.slot(h)`, an `Option<i32>`, and the program indexes `pool.values` with it, which counts in its lines.

### A second amendment, and what writing the workloads for p1 found (2026-11-24, before any measure)

To compare times, every option must compute the same. In p0, a spark's parent and a node's new parent were drawn by slot; a pool never shows its slots, so a user of p1 draws among handles, and computes something else. **W1's parent is now the spark born just before; W2's new parent is drawn from the list of every node inserted, in order, the root when that one was removed.** p0 keeps that list by hand, with each node's number, which its lines count.

Written for p1 (`bench/pool/p1/`), W1 and W2 met five errors, all from one friction that is not the pools': an option is not narrowed inside a condition joined by `&&` or `||` (`p.isSome() && sparks.has(p)`); split into two `if`, they build. W3 and the stale read built first time. Under p1, W1, W2 and W3 give what p0 gives, twice on one instance, and the stale read gives `-1`, caught; under p2 the stale read gives `99`, W1 and W2 give other numbers, and **W2 of 100,000 nodes never ends**: a handle of a removed parent names the new value of its slot, which can be one of its own descendants, and the walk up the parents turns in a circle.

### The prototypes (2026-11-24)

- **p1 and p2** (`PERAS_PROTO_POOL=p1|p2`): the compiler adds `Pool<T>` to a program that names it, its source written in Peras (`crates/perasc-driver/src/pool/p1.peras`, `p2.peras`); the checker types `pool[h]`, a pool indexed by a handle, as its element (`Analysis::pool_indexes`), and the driver writes each one again as `pool.values[pool.slotOrFault(h)]`, then checks the program again, so that every path that reads or writes a place builds it as it builds an array's element. **Departure**: "built into the compiler" is built as a source the compiler adds and a rewriting, not as a new type of the lowering with its own runtime; a lowering of its own, its generation in the same column as its value, was not built, and its speed is not measured here.
- **p3** (`PERAS_PROTO_POOL=p3`): `import { Pool, Handle } from "peras:pool"` becomes blanks of its length, and the pool's source (`pool/p3.peras`) is added after the program; a value is reached through `slot(h)`.
- Each passes `cargo test --workspace` with its variable set and the differential test with `PERAS_DIFFERENTIAL_SEEDS=2000`, its generator writing a pool whose values are inserted, removed and read through handles kept across removals. **Departure**: that test compares the IR as written with the IR optimized, the pool's source on both sides, so it cannot see a pool written wrong (finding M026); `scripts/bench-pool.mjs --check` sees it: p1 with its generation not compared reads the stale handle as `99`, and the check fails.

### The measure (2026-11-24)

At `5194fdb`, `node scripts/bench-pool.mjs`, 21 rounds ([results](../../../bench/pool/RESULTS.md)); then `check-regression` against `d7bf515` for each option.

|     | W1, the sparks | W2, the scene  | W3, the chunks | W4               | memory, W1 and W2    | the stale read |
| --- | -------------- | -------------- | -------------- | ---------------- | -------------------- | -------------- |
| p0  | 1.000          | 1.000          | 1.000          | 1.000            | 1,856 KiB, 6,464 KiB | 99, in silence |
| p1  | 2.237          | 1.421          | 1.215          | identical module | 2,496 KiB, 7,744 KiB | **-1, caught** |
| p2  | another result | **never ends** | 1.084          | identical module |                      | 99, in silence |
| p3  | 1.441          | 1.424          | 1.126          | identical module | 2,496 KiB, 7,744 KiB | **-1, caught** |

Lines of W1, W2 and W3, blank lines and comments apart: p0 63, 70, 56; p1 and p2 39, 51, 57; p3 45, 59, 61. Errors met: p0, six `E0210` and an overflow in a development build; p1, five, all of one friction that is not the pools' (finding F118); p3, none, written after p1. `check-regression`: of 260 modules, none larger and none changed under any option; the compiler 1.025, 1.025 and 0.991 times its time.

Against the criteria registered:

- **p2 is out**: no safer than p0, and on W2 it never ends: a removed parent's handle names its slot's new value, which can be one of its own descendants.
- **p1 is out**: safer, and shorter on W1 and W2, but 1.215 to 2.237 times p0's time, above 1.05, its memory 1.34 times on W1 and 1.20 on W2, above 1.10 and more than a page, and one line more on W3.
- **p3 is out**: safer, but 1.126 to 1.441 times p0's time, the same memory as p1, and five lines more on W3.

### Proposal

**p0 stays**: no option meets the criteria registered. The prototypes are removed, their figures kept here and their code in git at `5194fdb`. What the measure teaches: a handle that carries its generation catches every stale read, and a program written with one is shorter where values point to one another (W1, W2); built over arrays in Peras, its check paid on every access costs 13 to 124 % of time and 20 to 34 % of memory on these workloads. p1, rewritten as `slotOrFault`, checks the generation again at each `pool[h]`, up to nine per spark in W1, where p3 checks it with `slot(h)` once per spark and once per parent; that this explains their difference on W1 was not measured. A pool lowered by the compiler itself, its generation kept beside its value and checked once per reach, was not built and is not ranked by a guess (finding M001); it is the option to measure if the safety of handles is wanted again. `@frame` and the study of proximity, which wait for a fixed budget of slots, can take arrays and indices as p0 writes them, as W3 does. The owner decides.

### A third amendment: p4, the pool the compiler walks and checks (2026-11-25, before any prototype of it)

Asked by the owner on 2026-11-24, after decision 0045 landed: the measure above names, as the option to measure, a pool whose generation the compiler checks once per reach. Read in the code before writing this (finding M020): p1 checks the generation at each `pool[h]`, up to nine times per spark in W1, through `slotOrFault`, and walks the pool through `handles()`, which builds an array of every live handle at each frame; p0 walks its slots in place and checks nothing it does not test.

- **p4. p1, and two things the compiler does itself** (`PERAS_PROTO_POOL=p4`): `for (const h of pool)` walks the slots in place, the handle of each live one made as it is reached, no array built, a value inserted during the walk reached too, as a `Map` is walked; and in the body of such a loop, the generation of its handle is checked once, before the first statement that reads through it, and checked again only after a statement that may insert or remove: one that calls a method of the pool other than `has`, `get` and `size`, or passes it to a function. A stale handle still stops the program where it indexes, since the slot found is `-1` and indexing with it stops; the check is moved, not dropped.
- **Not built: the generation beside the value.** An array of structures is stored as columns (the context above), so a slot holding its generation and its value would still be two buffers; putting them in one is a change of the layout of every array of structures, not of the pool, and is not ranked by a guess (finding M001).
- **Departure, as for p1**: built as a rewriting of the program's tree and the pool's Peras source the compiler adds, not as a new type of the lowering.
- **The workloads for p4** (`bench/pool/p4/`) are p1's, each `for (const h of pool.handles())` written `for (const h of pool)`, and nothing else changed: W1's walk of the sparks, W2's sum of the depths; W3 keeps its `handles()`, which it indexes.
- **The measure** is run again whole, every option at one commit after decision 0045, since the compiler changed under p1 and p3; the figures above, at `5194fdb`, are kept. The protocol, the workloads and the criteria are those registered above, unchanged; the correctness checks too, p4's mutation being the check made once and never again after a statement that removes. _Corrected on 2026-11-25, before p4's prototype_: as registered, this said the differential test must see that mutation fail; it cannot, since it compares the program as rewritten with the same program optimized, both sides rewritten alike (finding M026). What sees it is `scripts/bench-pool.mjs --check`, on a program that removes a handle in the body of a walk and reads through it after (`bench/pool/p4/stale-walk.peras`): under p4 it stops, and with the mutation it reads in silence.

### p4's prototype, and the measure again (2026-11-25)

p4 is prototyped as the third amendment defines it, at `49c5842`: the checker types `for (const h of pool)` and records the walk (`Analysis::pool_loops`); the driver writes it again as a walk over the pool's slots and checks `h`'s generation once in its body, before the first statement that reads through it, and again after a statement that may insert or remove (`walk` and `changes_pool` in `crates/perasc-driver/src/pool.rs`). In W1 the check is made once per spark, where p1 makes it up to nine times; the `if` that removes keeps the slot found in its condition. p4 passes `cargo test --workspace`, the end-to-end tests and the differential test with `PERAS_DIFFERENTIAL_SEEDS=2000` with its variable set, the differential test walking a pool that reads, writes and removes. On `bench/pool/p4/stale-walk.peras` it stops at the stale handle (`index -1 is out of bounds`); with its check never made again, the mutation registered, it reads 456 in silence, and `scripts/bench-pool.mjs --check` fails.

At `49c5842`, every option measured again in one process, 21 rounds ([results](../../../bench/pool/RESULTS.md)); then `check-regression` against `aff5431`, the merge before p4, for p1, p3 and p4.

|     | W1, the sparks | W2, the scene | W3, the chunks | W4               | memory, W1 and W2        | stale read; in a walk     |
| --- | -------------- | ------------- | -------------- | ---------------- | ------------------------ | ------------------------- |
| p0  | 1.000          | 1.000         | 1.000          | 1.000            | 1,856 KiB, 6,464 KiB     | 99, in silence            |
| p1  | 2.156          | 1.522         | 1.293          | identical module | 2,496 KiB, 7,744 KiB     | -1, caught                |
| p2  | another result | never ends    | 1.151          | identical module |                          | 99, in silence            |
| p3  | 1.379          | 1.480         | 1.218          | identical module | 2,496 KiB, 7,744 KiB     | -1, caught                |
| p4  | **1.246**      | **1.413**     | 1.249          | identical module | **2,240 KiB, 7,232 KiB** | -1, caught; stops, caught |

p4's W3 builds into p1's module, byte for byte (it has no walk over a pool), so their two figures there, 1.249 and 1.293, measure the same module twice. Lines are p1's: 39, 51 and 57.

`check-regression`: of 280 modules, none larger, none changed, under each option. The compiler's time: p3 1.025, but p1 **1.056** and p4 **1.059**, above 1.05, and the script fails for both. **Departure**: p1's code did not change since `5194fdb`, where the same ratio was 1.025; one measure of the compiler's time varies by about 0.03 here. It does not decide p4, which the criteria rule out on its own figures.

Against the criteria registered:

- **p4 is out**: safer than p0, in both reads, but 1.246 to 1.413 times p0's time, above 1.05, and its memory 1.21 times p0's on W1 and 1.12 on W2, above 1.10 and more than a page. It is the best of the safe options: checking once per walk takes W1 from p1's 2.156 to 1.246, and not building the array of handles takes 256 KiB from W1's memory.
- p1, p2 and p3 are where the first measure put them.

### Proposal, after p4

**p0 stays**: no option, p4 included, meets the criteria registered. What p4 teaches: of p1's extra time on the sparks, about four fifths went once the compiler checked the generation once per walk and built no array of handles, two changes made together and not measured apart (finding M013); what is left, 25 to 41 % of time and 12 to 21 % of memory, is the layout itself: a handle of two numbers in an option where p0 keeps one, and the columns of generations and of live slots. A pool as safe as p1 costs that much over indices by hand on these workloads. The prototypes are removed once the owner decides, their figures kept here and their code in git at `49c5842`.

### Decision and landing (2026-11-25)

The owner kept **p0**, and with this decision adopted three rules of method that it and decision 0043 taught: an option's floor measured before it is built, the noise of every protocol measured by an A/A control, and two changes an option combines measured apart (findings M041 to M043). The prototypes are removed: `crates/perasc-driver/src/pool.rs` and the pool's Peras sources, `Analysis::pool_indexes` and `pool_loops`, the checker's typing of `pool[h]` and of a walk over a pool, the differential test's pool statements, and `scripts/bench-pool.mjs`, all kept in git at `49c5842`, where the measure can be run again; `bench/pool` keeps the workloads and the results. `check-regression` against `94b0a39`: of 280 modules, none larger, none smaller, none changed; the compiler 1.016 times its time and 33,416 bytes smaller. A program keeps writing what W1 to W3 write for p0, indices by hand, which `@frame` and the study of proximity can take up as they are.
