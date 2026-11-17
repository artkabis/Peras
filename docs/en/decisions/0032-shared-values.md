# 0032: Shared values, `Rc<T>`

_French translation: [../../fr/decisions/0032-shared-values.md](../../fr/decisions/0032-shared-values.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks. It answers finding F310, and the shared mesh that [decision 0029](0029-pond-of-lanterns.md) found refused. It comes after decisions [0030](0030-numeric-literals.md), on numeric literals, and 0031, on threads, in the order the owner set._

## Context

What the compiler does today, read before writing this (finding M020):

- **A structure is its fields, flattened.** In an owned module, a structure has no block of its own: the lowering gives each of its scalars a local, a structure inside a structure being inlined field by field (`leaves_in`, `crates/perasc-ir/src/lower/values.rs`). A type that contains itself, as `struct Tree { children: Array<Tree> }` or `enum List { Nil, Cons(i32, List) }`, has no end there: it is checked, not compiled (`E0900`, "types that contain themselves", finding F310).
- **Every block of the heap has one owner.** The ownership pass (`crates/perasc-check/src/owner.rs`) moves a string, an array, a map or a set on assignment, return, `sink` or storage in a structure, and lends it otherwise; a second holder needs `copy()`. `Rc<T>` is not a known type: the specification promises it ("sharing only through an explicit `Rc<T>`, visible in the type") and leaves it to a decision.
- **One counted block exists already.** A function value's environment ([decision 0026](0026-closures.md)) carries a count of its holders: a copy counts one more, the last to let go frees it, a static environment is never counted. It is the first and only reference count of the runtime, single-threaded, and its environments never point to one another.
- **A turn of `parallel for` may not call, allocate or reduce** (`E0241`, finding F301); decision 0031 may widen it before this one.
- **The pond's shared mesh is refused**: a lantern holding the mesh a thousand lanterns share, `lanterns.push(Lantern(mesh, …))` in a loop, meets `E0204`; `mesh.copy()` would keep a thousand copies (`bench/lanterns/shared-mesh.peras`).

What the languages without a collector teach, and what the owner raised on 2026-11-17: a counted pointer that can be changed lets a program build a cycle, which no count frees; every copy and release touches the count; and a count changed by two threads at once needs atomic operations. Swift's arrays share their buffer and copy it on the first change while it is shared (copy on write). Perceus (Reinking, Xie, de Moura and Leijen, PLDI 2021) and _Counting Immutable Beans_ (Ullrich and de Moura, IFL 2019) show a compiler eliding most count operations, by lending what is only read and reusing what is released.

## Options

- **r0. No `Rc`**: sharing by an index into an array, the mesh kept once and each lantern holding its number, as `bench/lanterns/pond.peras` does. The reference.
- **r1. `Rc<T>` read only, copied on write.** A shared value cannot be changed in place: to change it, the program asks for its own (`rc.make()` or a field written), done in place when the count is 1, after a copy otherwise. A value can then only point to values that existed before it: no cycle can be built, and there is no `Weak`. Counts change only where an `Rc` is stored while its source is kept, and where its last holder ends; lending and moving touch none. Cloning or releasing an `Rc` in a turn of `parallel for` is refused, so a plain count is enough, never an atomic one.
- **r2. `Rc<T>` that can be changed, with `Weak<T>`**, as Rust's `Rc<RefCell<T>>` and `Weak`: cycles can be built, and are broken by hand with a weak reference, which does not keep its value alive.
- **r3. `Rc<T>` that can be changed, with a detector of cycles** at release, by trial deletion (Bacon and Rajan, ECOOP 2001): a count that falls but not to zero marks its value as a possible root, and the roots are scanned from time to time. Built only if it can be within this decision; otherwise reported as not built, with why (finding M001).

## Protocol, registered before any prototype

Four quantities, as for decisions 0027, 0028 and 0030, and three questions of their own. Each option is prototyped behind `PERAS_PROTO_RC`; the build without it keeps today's behaviour, r0.

1. **Ease of writing**: four programs, written as a user writes them from JavaScript, then fixed until they build, for each option:
   - P1, the pond's shared mesh, `bench/lanterns/shared-mesh.peras` as written;
   - P2, a binary tree built from a list of numbers and summed, the types that contain themselves of F310;
   - P3, a scene graph whose nodes point to their parent, the case of a cycle;
   - P4, a list held by two holders, then changed through one: the other must not see the change.

   For each: the errors met before it builds, with their codes, the lines changed from the JavaScript, the notions JavaScript does not ask for.

2. **Workloads**, owned modules on TLSF in Node, each option's module and r0 in one process, the order alternating, 21 rounds, the median of the ratios to r0 round by round and their range (finding M017):
   - **W1**, the pond with each lantern holding its mesh, against the version by indices: frame 600 at 1024 × 768 with 1,000 lanterns;
   - **W2**, a tree of 1,000,000 nodes built, summed, then released.
3. **Load on the processor**: the CPU time over the wall time of each module's rounds, as decision 0012 measures it.
4. **Memory and bytes**: the module's memory after each workload; its bytes, raw and gzipped.
5. **Leaks possible or not**: P3 run on the debug allocator; the blocks left alive once its last holder ends. An option that lets P3 build a cycle the program does not break leaks; how many blocks is reported.
6. **Count operations per frame of the pond**: the counts incremented and decremented while W1 draws a frame, counted by the debug allocator. The target is 0 in the loop that draws: a lantern's mesh is taken once, when the lantern is made.
7. **Usable in `parallel for` or not**: whether a turn may read a shared value, and what the checker says of a turn that clones or releases one.
8. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-shared.mjs` runs them; `--check` checks that every program builds and leaves nothing alive where its option says so, and is run with every check of the project.

## Record

_Each part is recorded here as it lands._
