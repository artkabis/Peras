# 0044: Pools, many values of one type held by handles

_French translation: [../../fr/decisions/0044-pools.md](../../fr/decisions/0044-pools.md)._

_Status: protocol registered on 2026-11-24, before any prototype and any measure, as the rule of the project asks. It is the step of phase 6 of the [roadmap](../reference/roadmap.md) that follows the arenas ([decision 0043](0043-arenas.md)), "the `Pool<T>` allocator", and comes before `@frame` and the study of memory streamed by proximity, which needs a fixed budget of slots. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression; if none is, the decision says so and nothing lands._

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

_To come: the definitions tested, the prototypes, the measure, the proposal._
