# 0033: Shared values in a managed module

_French translation: [../../fr/decisions/0033-shared-values-in-managed-modules.md](../../fr/decisions/0033-shared-values-in-managed-modules.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks. It follows [decision 0032](0032-shared-values.md), whose option r1, `Rc<T>` read only and copied on write, owned modules compile; a managed module refuses `Rc` (E0243). The owner asked for this step on 2026-11-17._

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
