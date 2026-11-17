# 0036: Small functions inlined, invariants moved out of branches

_French translation: [../../fr/decisions/0036-inlining-and-invariants.md](../../fr/decisions/0036-inlining-and-invariants.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks. It answers finding F222, which [decision 0029](0029-pond-of-lanterns.md)'s profile found: in the pond, the work that depends on the row only is done at every pixel, and small functions are called where LLVM inlines them; done by hand, each gains about 3.5 % of the frame. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

## Context

What the compiler does today, read before writing this (finding M020):

- **No function is inlined**, but a recursive one into itself, three levels deep (`crates/perasc-ir/src/inline.rs`). The pond's `tri`, `unit` and `byte`, a few operations each, are calls at every pixel.
- **Loop-invariant code motion** (`crates/perasc-ir/src/licm.rs`) moves `l = e` out of a loop when `e` is simple (`is_simple`, `crates/perasc-ir/src/opt.rs`): no call, and no division at all, a float's included, which cannot trap. An assignment inside an `if` of the loop does not move.
- **A function that changes nothing is known** (`pure_functions`, decision 0031), for `parallel for`; nothing else uses it.
- **The engine inlines on its own** too, by its own budget (finding of `inline.rs`): a gain here must be measured, not assumed.

## Options

- **i0. As today.**
- **i1. Small functions inlined**: a function of the module whose body is one `return e`, `e` no larger than 40 nodes, without loop, call to itself or value of the heap, is replaced at each call by `e`, its parameters bound to the arguments, each evaluated once and in order.
- **i2. Wider invariants**: a float division counts as simple; so does a call to a function that changes nothing and cannot trap; and an invariant assignment inside an `if` of the loop moves out of the loop when its expression is simple, being then safe to compute once whatever the branch.
- **i3. Both**, i1 then i2.

## Protocol, registered before any prototype

Each option is prototyped behind `PERAS_PROTO_INLINE`; the build without it keeps today's behaviour, i0.

1. **Correctness**: the differential test (`crates/perasc-driver/tests/differential.rs`) with `PERAS_DIFFERENTIAL_SEEDS=2000` under each option, its generator extended with what the options touch (small functions called in loops, divisions, branches); then once with each option broken on purpose, to see the test fail (finding M015). Every `--check` script of the project under each option.
2. **Speed and load on the processor**, owned modules on TLSF in Node, each option's module and i0's in one process, the order alternating, 21 rounds, the median of the ratios to i0 round by round and their range (finding M017), the CPU time over the wall time:
   - **W1**, the pond, frame 600 at 1024 × 768 with 1,000 lanterns (`bench/lanterns/pond.peras`);
   - **W2**, the garden of decision 0013 (`bench/garden/garden.peras`), one frame;
   - **W3**, each kernel of `bench/browser/kernels.peras`.
3. **Bytes**: each module of the repository, raw and gzipped, by `scripts/check-regression.mjs` under each option, which also gives the compile time and the compiler's size. An optimization changes modules by design: every module that changes is one of W1 to W3, or is measured alongside them, and none may be slower (the owner's rule).
4. **Ease of writing**: none to measure; the same program is written for every option. Reported as such.
5. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-inline.mjs` runs points 2 and 3; `--check` checks that every option's modules give i0's results, and is run with every check of the project.

## Record

_Each part is recorded here as it lands._
