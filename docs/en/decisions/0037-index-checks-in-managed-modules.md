# 0037: How a managed module checks its indices

_French translation: [../../fr/decisions/0037-index-checks-in-managed-modules.md](../../fr/decisions/0037-index-checks-in-managed-modules.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks; options measured at `338eceb`; proposal: o1. It answers finding F221, which [decision 0029](0029-pond-of-lanterns.md)'s profile explained: the pond's managed module, built for production, takes 2.62 times TypeScript's time in Node, and 1.09 once its index checks are written as direct accesses. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

## Context

What the JavaScript generator does today, read before writing this (finding M020):

- **Every index not proven in bounds goes through one of two functions** of the module (`crates/perasc-codegen-js/src/runtime.rs`): `$at(xs, i)` reads, `$put(xs, i, v)` writes; out of bounds, both throw the `RangeError` of the owned module. An index the generator proves in bounds (`proven`, a counted loop over the array's length) is read directly.
- **The same two functions serve every array of the module**: arrays of numbers, of strings, of structures, typed arrays from JavaScript (an `inout Array<f64>` parameter may be a `Float64Array`).
- **The check is the language's promise**: an index out of bounds stops the program in both modes, never gives `undefined`.

## Options

- **o0. As today**: `$at` and `$put`.
- **o1. The check written at each access**: `(i >>> 0 < xs.length ? xs[i] : $bounds(i, xs.length))` for a read; for a write, the value computed first, as `$put` computes it, then the same test before the assignment. A part that is not a local variable is kept in a temporary, so that each is evaluated once and in order.
- **o2. One pair of functions per kind of element**, by the type the checker gives the array: numbers, integers, booleans, strings, other values; the same code, a function each.
- **o3. No check in a production build**: `xs[i]` and `xs[i] = v`. Built to measure what the check costs, not proposed: an index out of bounds gives `undefined`, against the language's promise.

## Protocol, registered before any prototype

Each option is prototyped behind `PERAS_PROTO_INDEX`; the build without it keeps today's behaviour, o0. Every module measured is a production build, named (`--allocator tlsf`, finding M029).

1. **Correctness**: every `--check` script of the project and `pnpm run e2e` under each option; an end-to-end test reading and writing out of bounds, in reads, writes, compound assignments (`xs[i] += v`) and steps (`xs[i]++`), whose index and value have effects whose order counts: o0, o1 and o2 throw the same error after the same effects; what o3 does is recorded.
2. **Speed and load on the processor**, in Node, TypeScript or JavaScript written by hand and every option's module in one process, the order alternating, 21 rounds, the median of the ratios round by round and their range (finding M017), the CPU time over the wall time:
   - **W1**, the pond, frame 600 at 1024 × 768 with 1,000 lanterns, against `pond.ts`;
   - **W2**, the garden of decision 0013 as a managed module, one frame at 1024 × 1024, against its TypeScript version;
   - **W3**, each kernel of `bench/managed/kernels.apeiron`, against the JavaScript written by hand of `scripts/bench-managed.mjs`.
3. **Bytes**: every managed module of the repository, raw and gzipped, by `scripts/check-regression.mjs` under each option, which also gives the compile time and the compiler's size.
4. **Ease of writing**: none to measure; the same program for every option. Reported as such.
5. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-index.mjs` runs points 2 and 3's workloads; `--check` checks that every option's modules give the reference's results, and is run with every check of the project.

## Record

_Each part is recorded here as it lands._

### Correctness (2026-11-17)

- **A test reads and writes out of bounds** (`an_index_out_of_bounds_throws_after_the_same_effects`), in a read, a write, a compound assignment and a step, its index and value with effects: o0, o1 and o2 throw the same `RangeError` after the same effects (a read after its index, a write after its index and value, a compound assignment after its index, its read failing first). A mutation of o1 computing the value after the test fails it.
- **o3, as foreseen, breaks the language's promise**: the read gives 0 (`undefined + 1`, wrapped), the write grows the array to six elements, both silently; it fails that test and the older `out_of_bounds_throws`.
- **Every `--check` script, the end-to-end tests and `cargo test --workspace`** pass under o1 and o2. Under o3, the two tests above fail, as expected. One run of `bench-workers.mjs --check` under o3 failed without a message; it builds no managed module, so no option of this decision reaches it, and nine runs after it, three under o3 and six under none, passed. Recorded here, not explained.

### The measure (2026-11-17)

Two runs at commit 338eceb, one after the other, nothing else running, every module a production build; every table in [`bench/index/RESULTS.md`](../../../bench/index/RESULTS.md), with the machine. Ratios to the reference written by hand, median of 21 rounds, run 1 then run 2:

| Workload                                 | o0, today      | o1             | o2             | o3, no check   |
| ---------------------------------------- | -------------- | -------------- | -------------- | -------------- |
| W1, the pond, against `pond.ts`          | 2.402, 2.598   | 1.186, 1.144   | 2.203, 1.987   | 0.978, 0.922   |
| W2, the garden, against `garden.ts`      | 1.272, 1.316   | 1.308, 1.306   | 1.285, 1.306   | 1.287, 1.326   |
| W3, the four kernels, against JavaScript | 0.787 to 1.076 | 0.995 to 1.037 | 0.792 to 1.092 | 0.929 to 1.000 |
| The pond's module, bytes (gzip)          | 14,569 (3,674) | 18,017 (4,149) | 14,875 (3,694) | 13,889 (3,513) |

- **o1 halves the pond's time**, from 2.40 to 1.19 times TypeScript's and from 2.60 to 1.14, the check kept. What is left to TypeScript, about 0.2, is what checking costs: o3, which checks nothing, draws at 0.98 and 0.92.
- **o2 does not**: a pair of functions per kind of element leaves the pond at 2.20 and 1.99. Why one function per access helps where one per kind does not is not measured here; that the engine optimizes each site apart is a possible cause, not a shown one (finding M001).
- **The garden is 1.27 to 1.33 times TypeScript's time under every option**: its one checked access is not its gap, which is elsewhere (finding F224).
- **W3 is within the noise of the method**: `words` reads no index, its code the same byte for byte under o0, o1 and o2, and still gives 0.79 to 1.03.
- **Bytes**: by `scripts/check-regression.mjs 5ba74b7`, o1 makes no module of the repository larger and two smaller (`bench/managed/kernels.apeiron`, 2,756 to 2,725 bytes; `bench/web-bugs/w11-off-by-one.apeiron`, 977 to 948); o2 makes those two larger; o0 and o3 change none, the script building managed modules for debug, where o3 still checks. The pond is no managed module of the repository, and the script does not see it: o1 makes it 24 % larger, 13 % gzipped, each checked access written out. Compile time from 0.973 to 1.004; the compiler grows by 4,736 bytes with the four built.

### Proposal

**o1**: the pond's managed module halves its time and keeps its check, at 1.14 to 1.19 times TypeScript's, every result and every error the same as today's; no module of the repository grows, two get smaller. Its cost, 475 more gzipped bytes in a module that checks 64 accesses, is the pond's; the owner's rule, as `check-regression.mjs` holds it, is met. The owner decides.
