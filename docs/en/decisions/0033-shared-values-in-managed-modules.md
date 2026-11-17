# 0033: Shared values in a managed module

_French translation: [../../fr/decisions/0033-shared-values-in-managed-modules.md](../../fr/decisions/0033-shared-values-in-managed-modules.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks; options measured at `86de5c1`; option m3 accepted by the owner on 2026-11-17, under the rule that every decision is an improvement, never a regression. It follows [decision 0032](0032-shared-values.md), whose option r1, `Rc<T>` read only and copied on write, owned modules compile; a managed module refuses `Rc` (E0243). The owner asked for this step on 2026-11-17._

## Context

What the compiler does today, read before writing this (finding M020):

- **A managed module shares its structures.** It is JavaScript: a structure is an object, and `let b = a; b.n = 5;` changes the object `a` names too (`two()` gives 455, `crates/perasc-codegen-js`). An owned module refuses that program (E0204, `a` moved): no program builds in both modes and disagrees.
- **An owned module's `Rc<T>` is a value.** Writing through one holder gives that holder its own value (decision 0032): `let b = a; b.items.push(4);` leaves `a` with three items. To agree, a managed module must copy somewhere.
- **`copy()` of a structure is `structuredClone`** (`crates/perasc-codegen-js/src/methods.rs`); a managed module emits no release: what a holder holds goes when the garbage collector sees no one holds it.
- **The tests compare both modes**: a program that builds in both must give the same result (the end-to-end tests of decisions 0028 and 0030).

## Options

- **m0. Refused**, E0243, as today: a managed module that needs a shared value imports it from an owned one.
- **m1. Shared, copied at every write.** `Rc.new(v)` is `v`; a copy of an `Rc` is the same object; a write through a holder, a field written or a place changed through it, first gives the holder a copy, `b = structuredClone(b)`, whatever holds it.
- **m2. Copied at every holder.** A copy of an `Rc`, an assignment, a field or an element made from one, is a copy of the object; a write is in place. Nothing is shared: a thousand lanterns hold a thousand meshes.
- **m3. A box counted, never released.** An `Rc` is `{ v, n }`: a copy holds the same box and adds one to `n`; a write through a holder whose box has `n > 1` takes one from it and gives the holder a new box, `{ v: structuredClone(v), n: 1 }`. A holder that ends takes nothing off: the count can only be too high, so a copy may be made that was not needed, never one that was missed.

## Protocol, registered before any prototype

Each option is prototyped behind `PERAS_PROTO_RCM`; the build without it keeps today's behaviour, m0.

1. **Agreement**: every function of `packages/vite-plugin-peras/e2e/fixtures/shared/shared.peras`, and P1 to P4 of decision 0032 as fixed for r1, built as a managed module, give what the owned module gives. An option that disagrees anywhere is out.
2. **Workloads**, managed modules in Node, each option's module in one process, the order alternating, 21 rounds, the median of the ratios round by round and their range (finding M017), against the owned module of r1 on TLSF for scale:
   - **W1**, decision 0032's pond with each lantern holding its mesh, frame 600 at 1024 × 768 with 1,000 lanterns;
   - **W2**, decision 0032's tree of 1,000,000 nodes, built and summed;
   - **W3**, 1,000,000 writes through one holder of a value two hold: `a.value += i` in a loop, `b` holding it too.
3. **Load on the processor**: the CPU time over the wall time of each module's rounds.
4. **Memory and bytes**: the JavaScript heap after each workload, after a collection; the module's bytes, raw and gzipped.
5. **Ease of writing**: the programs of point 1 as written for the owned module; the errors each option meets, and the lines changed.
6. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-shared-managed.mjs` runs them; `--check` checks the agreement of point 1, and is run with every check of the project.

## Record

_Each part is recorded here as it lands._

### The prototypes (2026-11-17)

Built behind `PERAS_PROTO_RCM` in the JavaScript generator (`crates/perasc-codegen-js/src/shared.rs`): a shared value read to be held once more is wrapped, under m2 in `structuredClone`, under m3 in `$hold`; read as the base of a field or the receiver of a method, it is lent; a field written, or a place on the way to a write (`Analysis::changed`), makes the value its holder's own first, under m1 by `structuredClone`, under m3 by `$own`, each shared value on the way, outermost first. Every option agrees with the owned module on the end-to-end fixture of `Rc` and on P1 to P4, as written for the owned module: no line changes. m0 refuses each with E0243.

### The measure (2026-11-17)

Run 1, at commit 86de5c1, nothing else running; every table in [`bench/shared/MANAGED.md`](../../../bench/shared/MANAGED.md), with the machine. Ratios to the owned module of r1, median (range):

| Workload                                        | m1                           | m2                           | m3                      |
| ----------------------------------------------- | ---------------------------- | ---------------------------- | ----------------------- |
| W1, the pond                                    | 9.538 (7.916 to 10.613)      | 9.870 (8.601 to 11.657)      | 9.613 (7.882 to 12.101) |
| W2, a tree of 1,000,000                         | 0.690 (0.375 to 0.981)       | 136.451 (111.594 to 151.500) | 1.568 (1.226 to 1.977)  |
| W3, 1,000,000 writes through one of two holders | 387.544 (364.229 to 458.179) | 1.816 (1.641 to 4.125)       | 1.490 (1.230 to 2.634)  |
| CPU / wall, W2                                  | 1.86                         | 1.07                         | 2.61                    |
| Bytes of W3 (gzip)                              | 610 (395)                    | 597 (393)                    | 791 (466)               |

- **W1 does not separate them**: the pond is drawn by arithmetic in every option, its mesh only read; a managed module takes about 9.6 times the owned module's time on this frame whatever the option, the cost of a managed module that decision 0029 measured (F221), not of sharing.
- **m2 copies a tree at every call**: `sum(node.left)` holds `node.left` once more, so its subtree is copied, at every level: 136 times the owned module.
- **m1 copies at every write**: a million writes are a million copies, 388 times the owned module.
- **m3 is never far**: 1.57 on the tree, 1.49 on the writes, where the box is read through and counted; the garbage collector working beside it, 2.61 cores busy for one on W2.
- **The JavaScript heap after a collection**, as registered, is the same for every option on every workload (5,865 to 6,816 KiB): what a round allocates is gone once collected. It separates nothing; the time does.

### Proposal

**m3**, a box counted and never released: it agrees with the owned module, never copies more than once a holder that writes, and stays within twice the owned module's time where m1 and m2 each fall by two orders of magnitude on one workload. Its cost: 194 more bytes than m1 on W3 (two helpers), and counts that only grow, so that a value once shared is copied at its first write even after its other holders ended. The owner decides.

### Option m3 lands (2026-11-17)

A managed module compiles `Rc<T>` as a box, every time; m1 and m2 are removed from the compiler, their figures kept above and their code at 86de5c1. `packages/vite-plugin-peras/e2e/shared.test.ts` now builds the fixture as a managed module too and checks every value against the owned module's; with `$own` never copying, that test fails. Against the compiler before decision 0032 (`node scripts/check-regression.mjs 42c7442`): every module of the benchmarks and examples identical, none larger; the compiler's time at 1.040 of its own; the compiler 52,024 bytes larger, the code of `Rc` in both modes.
