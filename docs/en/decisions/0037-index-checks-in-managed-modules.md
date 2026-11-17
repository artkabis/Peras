# 0037: How a managed module checks its indices

_French translation: [../../fr/decisions/0037-index-checks-in-managed-modules.md](../../fr/decisions/0037-index-checks-in-managed-modules.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks. It answers finding F221, which [decision 0029](0029-pond-of-lanterns.md)'s profile explained: the pond's managed module, built for production, takes 2.62 times TypeScript's time in Node, and 1.09 once its index checks are written as direct accesses. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

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
