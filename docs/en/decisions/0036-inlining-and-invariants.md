# 0036: Small functions inlined, invariants moved out of branches

_French translation: [../../fr/decisions/0036-inlining-and-invariants.md](../../fr/decisions/0036-inlining-and-invariants.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks; options measured at `1f4b070`; none chosen, i0 kept, by the owner on 2026-11-17. It answers finding F222, which [decision 0029](0029-pond-of-lanterns.md)'s profile found: in the pond, the work that depends on the row only is done at every pixel, and small functions are called where LLVM inlines them; done by hand, each gains about 3.5 % of the frame. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

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

### A precision of the protocol (2026-11-17), before any prototype

Read before the first prototype (finding M020): `optimize` (`crates/perasc-ir/src/opt.rs`) runs none of its passes, the motion of invariants included, on a function that uses the heap (`Function::uses_heap`), since no pass is proven to keep releases exact. The pond's `render` takes its arrays as parameters: today nothing moves out of its loops at all, not even a simple invariant. Every heavy function of W1 to W3 that takes an array is in that case.

The same holds for a function that calls one giving several results, a structure (`Function::uses_many`): `render` calls `lantern()`, which gives a `Mesh`.

So i2, and i3 with it, also runs the motion of invariants on a function that uses the heap or calls one giving several results, the targets of such a call counted as assignments, moving only an assignment to a local that is not a handle, of a simple expression, which reads no block (`is_simple` refuses `Buf`, `Move`, `Borrow` and `Alloc`): what it moves cannot allocate, release or read memory. i1 inlines into a function that uses the heap too, a small function taking and giving numbers or booleans only. The other passes of `optimize` stay as they are; the motion over a function that uses the heap is measured within i2, not as a fifth option, since without it i2 would not reach the pond.

### Correctness (2026-11-17)

- **The differential test**, its generator extended (four small functions; small functions called in loops, one with an argument read twice; a float division and a call that do not change in a loop, one inside a branch; arguments with effects whose order counts; a call on the right of `&&`; one in a loop's condition; a module variable read before a call that changes it): every option passes it with 2,000 seeds. Three mutations made on purpose fail it: i1 substituting every argument directly; i1 computing an argument into a variable where what comes before the call is not simple; i2 moving out of a branch an assignment that is not invariant.
- **Every `--check` script** of the project passes under i1, i2 and i3. `cargo test --workspace` passes under each but for two tests of `opt_tests`, which check i0's shape: `g(n)`, which gives `n`, moved out of a loop by i2, which it may, since `g` cannot trap; `g(n) = 10 / n` inlined by i1 where it was called, in the same order.

### The measure (2026-11-17)

Two runs at commit 1f4b070, one after the other, nothing else running; every table in [`bench/inline/RESULTS.md`](../../../bench/inline/RESULTS.md), with the machine. Ratios to i0, median of 21 rounds:

| Workload                   | i1             | i2             | i3             |
| -------------------------- | -------------- | -------------- | -------------- |
| W1, the pond, run 1, run 2 | 0.991, 1.014   | 1.060, 0.994   | 0.915, 0.964   |
| W2, the garden             | 1.046, 1.049   | 1.021, 1.009   | 1.053, 1.056   |
| W3, every kernel           | 0.966 to 1.107 | 0.925 to 1.018 | 0.761 to 1.006 |
| Modules larger, of 173     | 19             | 0              | 19             |
| Modules changed, same size | 2              | 26             | 9              |

- **W3 is the noise of the method**: the module of the kernels is the same, byte for byte, under the four options, and its ratios still run from 0.761 to 1.107 at the median. A difference of a few hundredths on W1 is within it. The CPU time over the wall time is 0.98 to 1.03 everywhere: one thread, as before.
- **Inlining slows the garden**, in both runs, i1 and i3 alike, every round of i3's second run: 5 %. Why is not measured. A cause is possible, not shown: `noise` grows by four copies of `unit`, and the engine, which sizes its own inlining by the bodies of the module ([`inline.rs`](../../../crates/perasc-ir/src/inline.rs)), may stop inlining it into `fbm`. A verdict on it would need a measure, not this reasoning (finding M001).
- **i1 and i3 make 19 modules larger**, by 1 to 3 %, which the owner's rule does not allow.
- **i2 moves what F222 found** out of the pond's loop of pixels, the row's divisions and the argument of `tri` (read in the IR), and changes no size; its gain on W1, if any, is under the noise of the method. The hand-written versions of decision 0029 gained 3.5 % each in their own profile; here, the engine seems to do part of it already, which this run cannot separate.
- **Compile time**, by `scripts/check-regression.mjs 1494bac`: 1.004 to 1.009 for every option; the compiler grows by 24,640 bytes with the four built.

### Proposal

**None of the options, i0 kept.** i1 and i3 make modules larger and the garden slower: a regression, under the owner's rule. i2 is no regression, but brings no gain the method can tell from its noise: an option that costs code and brings nothing measured is not an improvement. Once the owner decides, the prototypes leave the compiler, their figures kept here and their code in git at 1f4b070; the differential test keeps its wider generator. F222 stays open, with what this decision learned: an inlining for the pond would have to leave the engine's own inlining as it is, and its gain be shown above the noise, by more rounds or a workload where it is larger. The owner decides.

### Decision (2026-11-17)

The owner chose the proposal: **none of the options, i0 kept**. The prototypes left the compiler (`small.rs`, the wider motion of `licm.rs`, `PERAS_PROTO_INLINE`), and `scripts/bench-inline.mjs` with them, since it had nothing left to compare; their code is in git at 1f4b070, their figures above and in [`bench/inline/RESULTS.md`](../../../bench/inline/RESULTS.md). The differential test keeps its wider generator: it holds today's passes on small functions, divisions and branches too. F222 stays open.
