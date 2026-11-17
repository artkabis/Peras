# 0032: Shared values, `Rc<T>`

_French translation: [../../fr/decisions/0032-shared-values.md](../../fr/decisions/0032-shared-values.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks; options measured at `b30819e`; option r1 accepted by the owner on 2026-11-17, and the build's default since. It answers finding F310, and the shared mesh that [decision 0029](0029-pond-of-lanterns.md) found refused. It comes after decisions [0030](0030-numeric-literals.md), on numeric literals, and 0031, on threads, in the order the owner set._

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

### The prototypes (2026-11-17)

Built behind `PERAS_PROTO_RC`; the build without it is r0, today's compiler.

- **r1.** `Rc<T>` is one handle: the counted block of function values (decision 0026), holding the leaves of `T`, its buffers first (`EnvOp::Shared`). A copy holds it once more (`buf_clone`), a release lets go (`buf_release`), and the last holder releases what it owns. A field is read from its slot, a buffer lent. A field written, or a place changed through it (`rc.items.push(4)`, `rc.items[0] = 5`), first makes the value the holder's own: the runtime's `shared_unique` gives the same block when one holder holds it, a copy otherwise, its buffers copied and the shared values it holds held once more. The checker marks the way to every place changed (`Analysis::changed`). Not built: a write through a shared value held by another shared value or by an element (E0900, `rc-write`), and `Rc` in a managed module (E0900).
- **r2.** The same, written in place: every holder sees the write. `Weak<T>`, made by `Rc.weak(rc)` and read by `weak.get()`, an `Option<Rc<T>>`, is the value's address with its lowest bit set; the value's first word counts its weak holders, and its block goes with the last of its holders, strong or weak. The runtime's code for weak holders is in a variant of its own (`;; @r2` in `buffers.wat`, `Allocator::runtime_with`), linked only under r2: no other module carries it.
- **r3, not built.** A detector of cycles by trial deletion walks, from each possible root, the shared values it holds. The runtime cannot: a block records how many of its first slots it owns, not which are shared values, and an array or a map owns its handles without knowing whether they are shared values or strings. It would need a description of every type at run time, a buffer of roots and a scan, the core of a collector: more than this decision, and on top of r2, whose holes found below it would keep. Reported as not built, not ranked (finding M001).

Found while building them: F016 and F017, two bugs of the prototypes, closed before any measure; F114 and F115, two frictions of narrowing, met by P2 and P3.

What `node scripts/bench-shared.mjs --check` showed before the measure, and checks from now on:

- **P3 as written** gives, under r1, 45 where JavaScript gives 100: each child holds its parent as it was before the child was added, since a value can only point to values made before it. Under r2 it gives 100 and leaves 34 blocks alive, its cycle not broken; with a weak parent, none.
- **Two holders of one value**: a loop over `a.items` while `b` replaces `items` gives 6 under r1, as JavaScript, and 91 under r2, the loop reading the new array from its second turn on (`bench/shared/aliasing/loop.peras`).
- **In a turn of `parallel for`**, reading a shared value is accepted by both; holding it once more is refused by both (E0241); writing a field of it is refused by r1, where the write makes the value the holder's own and so assigns the variable, and accepted by r2, where several threads would write the same block.
- **W1 makes two count operations per lantern in the loop that draws**, under both: `const mesh = fleet[i].mesh` holds the mesh once more, then lets it go. The protocol's target was 0.

The measure, at the commit that lands the prototypes, is recorded next.

### The measure (2026-11-17)

Run 1, at commit b30819e, nothing else running; every table in [`bench/shared/RESULTS.md`](../../../bench/shared/RESULTS.md), with the machine.

| Quantity                                            | r0                           | r1                                          | r2                                               |
| --------------------------------------------------- | ---------------------------- | ------------------------------------------- | ------------------------------------------------ |
| P1, the shared mesh                                 | 9 lines, numbers in an array | builds as written                           | builds as written                                |
| P2, a tree                                          | 50 lines, an arena           | 9 lines (F114, E0205)                       | 9 lines (F114, E0205)                            |
| P3, parents                                         | 18 lines, numbers            | 12 lines: no cycle, counted from the parent | 6 lines, 34 blocks leaked; 13 with `Weak`, none  |
| P4, two holders                                     | 4 lines, `copy()`            | builds as written                           | 2 lines: the other holder sees the write         |
| W1, the pond, / r0                                  | 1                            | 1.012 (0.779 to 1.150)                      | 0.967 (0.770 to 1.140)                           |
| W2, a tree of 1,000,000, / r0                       | 1                            | 6.812 (4.555 to 7.482)                      | 6.797 (5.083 to 10.698)                          |
| CPU / wall                                          | 0.99                         | 1.00                                        | 1.00                                             |
| Memory after W2                                     | 16,448 KiB                   | 78,144 KiB                                  | 78,144 KiB                                       |
| Bytes of W1 (gzip)                                  | 14,652 (5,591)               | 14,783 (5,623)                              | 14,930 (5,674)                                   |
| Leaks possible                                      | no                           | no: no cycle can be built                   | yes, unless broken by `Weak`                     |
| Count operations in W1's loop                       | -                            | 2,000 a frame                               | 2,000 a frame                                    |
| `parallel for`                                      | -                            | reads; holding or writing refused           | reads; holding refused, writing accepted: a race |
| Two holders, a loop over a field the other replaces | -                            | 6, as JavaScript                            | 91                                               |
| r3                                                  |                              |                                             | not built                                        |

What it says:

- **Sharing costs nothing measurable where it is used as the pond uses it.** W1 under r1 and r2 is within the range of r0, whose own rounds vary as much; its 2,000 count operations a frame are lost in 50 ms of drawing.
- **A tree of shared nodes is 6.8 times slower than an arena, and holds 4.75 times its memory**, under both: each node is a block of its own, with its header, where an arena keeps three numbers in three arrays. `Rc` makes the type that holds itself possible (F310); it does not make it as fast as indices.
- **r1 and r2 run alike.** What separates them is what they let a program do: r2 leaks P3 as written, lets a loop see a write made through another holder, and lets the turns of a parallel loop write the same block. r1 refuses the last, gives JavaScript's result for the second, and cannot build the first, P3's cycle, which it asks to write from the parent.

### Proposal

**r1**, read only and copied on write: as fast as r2, no leak possible, no write seen through another holder, and nothing a parallel loop could race on; the values of Peras stay values (P4 as written). Its cost is P3: a graph that points back to its parents is written from the parent, or by numbers, as r0 writes it today. If the owner chooses it, the next steps are: to lend a shared value read in a loop rather than hold it once more (W1's 2,000 count operations a frame, the protocol's target being 0); F114 and F115; a write through a shared value held by another; and `Rc` in a managed module.

### Option r1 lands (2026-11-17)

Accepted by the owner. `Rc<T>` is in every build; `PERAS_PROTO_RC=r0` and `r2` build the other options measured, kept so that the measure can be run again, for `scripts/bench-shared.mjs` only. A program without `Rc` builds as before, byte for byte: the 214 builds of the benchmarks and examples compared with the compiler before this decision.

- **A shared value read in a loop is lent.** `const mesh = fleet[i].mesh` holds nothing when nothing from there to the end of the function changes or moves `fleet` or `i`: the checker records every place changed with the variable it starts from (`Analysis::mutations`), and the lowering reads the value from its place at each use (`lent_shared`). W1's count operations in the loop that draws fall from 2,000 a frame to 0, the protocol's target. A `let` still holds the value once more, which is what a turn of `parallel for` is refused (`bench/shared/parallel/holds.peras`).
- **Tested** by `packages/vite-plugin-peras/e2e/shared.test.ts` on both allocators: every case where lending would be wrong, a variable replaced, an index changed, an element replaced, the array grown or moved, gives a number of its own, and nothing is left alive. With the rule broken on purpose, mutations and moves ignored, both tests fail (finding M024).
- **Documented** in [the guide](../guide/memory.md#sharing-a-value), whose example is compiled by the tests of the docs.
- **A managed module refuses `Rc`** with E0243, as it refuses `sink` and `parallel for`, rather than reach code generation: the tests of the docs build every example in both modes, and the guide's met E0900 there.

Still to do, in this order: F114 and F115; a write through a shared value held by another shared value or an element; `Rc` in a managed module.

### Run 2, and findings F114 and F115 (2026-11-17)

Run 2, at commit 7155f9d, after lending: W1 under r1 at 1.033 of r0 (0.719 to 1.570), within the range of its own rounds as in run 1; its count operations in the loop that draws at 0; its module 206 bytes larger (14,989), the place a lent value is read from being written at each use. W2 at 6.131 (5.343 to 9.549). `bench/shared/RESULTS.md` holds the run.

F114 and F115, met by P2 and P3, are closed: a narrowed name or field is its option whole where the option is expected; `if (x.isSome())` narrows `x` in its block; a field of a `const` or a parameter is narrowed as a variable is. P3 as written now builds: under r1 it gives 45, as written, and is fixed as before; P2 as written meets only E0205, its parameter written.

### Writes through a shared value held by another or by an element (2026-11-17)

`b.left.n = 5`, `fleet[0].mesh.tris.push(4)`, `all[1].leaf.n++` build: the value is made its holder's own where it is held, the holder first when it is itself shared (`unique_handle`). The runtime does it in place, in a slot of the holder (`shared_unique_slot`) or in the column of the element (`shared_unique_elem`), so that a value no one else holds is not copied. Tested by `nested`, `inner`, `elements`, `elementFields` and `stepped` in the end-to-end test, on both allocators; without the two calls, `nested` gives 520521 for 110521. Building them found F018: a step through an `Rc`, and a field of an `Rc` an element holds, were lost without an error in the compiler of option r1 until now.
