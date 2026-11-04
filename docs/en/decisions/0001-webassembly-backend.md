# 0001: How `.peras` modules become WebAssembly

_French translation: [../../fr/decisions/0001-webassembly-backend.md](../../fr/decisions/0001-webassembly-backend.md)._

**Status:** accepted, 2026-11-04, with the amendments on guarantees below. The kickoff plan and the spec follow it.

## Context

The kickoff plan generates Rust from the IR, then builds it with `cargo` for `wasm32-unknown-unknown` and runs `wasm-bindgen`. That reuses LLVM's optimizer and makes `rustc` a second safety net, but every Peras user must install `rustup`, `cargo` and a matching `wasm-bindgen`, which is a large barrier for developers coming from TypeScript.

To decide on measurements rather than opinions, day 7 built both backends on the same IR:

- **direct**: `perasc-codegen-wasm` encodes the `.wasm` itself with `wasm-encoder`, and writes the JavaScript loader and the `.d.ts` declarations from the types it already knows;
- **rust**: `perasc-codegen-rs` writes a crate without `unsafe`, built by `cargo` and `wasm-bindgen`.

Both give integer arithmetic the same wrapping semantics, trap on the same divisions and saturate the same conversions.

## Measurements

From [`bench/backends/RESULTS.md`](../../../bench/backends/RESULTS.md), run by `node scripts/bench-backends.mjs` on a 4-core Xeon at 2.1 GHz, Node 26.13, Rust 1.99.1. The kernels are `fib` (recursion), `primes` (integer loops), `mandel` (`f64` loops) and `checksum` (`i64` arithmetic).

|                                         | direct   | rust                                        |
| --------------------------------------- | -------- | ------------------------------------------- |
| First build                             | 6.7 ms   | 9.2 s                                       |
| Rebuild after an edit                   | 4.7 ms   | 505 ms                                      |
| Tools needed                            | `perasc` | `perasc`, `rustup`, `cargo`, `wasm-bindgen` |
| Module size, raw                        | 615 B    | 19,232 B                                    |
| Module size, after `wasm-opt -O3`, gzip | 369 B    | 1,286 B                                     |
| `fib(32)`                               | 17 ms    | 4.2 ms                                      |
| `primes(1 000 000)`                     | 132 ms   | 131 ms                                      |
| `mandel(600)`                           | 16 ms    | 16 ms                                       |
| `checksum(5 000 000, 7)`                | 26 ms    | 11 ms                                       |

Timings vary by about 10 % between runs on this virtual machine; sizes do not. Every build returned the same results as a JavaScript reference on 19 inputs. For comparison, plain JavaScript took 20 ms on `fib(32)` and 164 ms on `checksum`, which needs `BigInt` since JavaScript has no 64-bit integers.

Where the differences come from:

- The Rust module embeds `dlmalloc`, Rust's allocator, although these functions never allocate: most of its size.
- On `fib`, LLVM turns one of the two recursive calls into a loop with an accumulator; the disassembled module shows it. `wasm-opt` does not do this, so it does not close the gap.
- On `checksum`, the cause has not been analyzed yet.
- On loops without such patterns (`primes`, `mandel`), V8's own optimizing compiler makes both equal.

## Decision

1. **The direct backend is the default**, for development and production: no tool to install beyond `perasc`, builds in milliseconds, the smallest modules.
2. **Peras gets its own optimization passes on the IR**, chosen and measured with `bench/backends`: first the accumulator transformation that explains `fib`, then the cause of the `checksum` gap once found. The IR knows what LLVM must prove, notably that `inout` arguments never alias.
3. **The Rust backend stays, required in CI and optional for users.** Behind `--backend rust`, it is a reference for performance until the direct backend matches it. In CI, every program of the backend benchmark goes through both backends, and they must return the same results: this differential test replaces the plan's rule that `rustc` must never reject a program `perasc` accepted. Users never need a Rust toolchain.
4. **Every generated module is validated** with `wasmparser` in the tests.

## Guarantees, and what protects them without `rustc`

The guarantees Peras gives its users come from `perasc`'s own checks, never from `rustc`: a user was never meant to see a `rustc` error. What `rustc` provided was a second net against bugs in `perasc` itself. Dropping it from the default path changes that net, not the user's guarantees.

**Typing.** Every type rule is checked by `perasc` before any code is generated, whatever the backend. Without `rustc`, a code generator bug producing ill-typed code is still caught by WebAssembly validation (in the tests, and by every engine at load time), but WebAssembly only has four value types: a `bool` used as an `i32`, a `u8` as an `i32`, or one structure as another would pass. To compensate:

- an **IR verifier** re-checks the types of the IR after lowering and after every optimization pass, as LLVM and Cranelift do;
- the **differential test** against the Rust backend catches a miscompilation that changes a result.

**Memory release.** The supported subset does not allocate yet, so nothing is released today. With the Rust backend, an `Array` would have become a Rust `Vec`: `rustc` would have inserted every release, on every path, and its borrow checker would have re-checked Peras's ownership analysis. With the direct backend, `perasc` inserts releases itself, and a bug in it could release twice, use released memory or leak. WebAssembly confines such a bug to the module's own memory: it cannot reach the page or the browser, but it can corrupt the module's data. Peras's model keeps this tractable: single owner, no stored references, exclusive `inout`, as in Swift or Hylo.

**Preconditions.** The direct backend compiles nothing that allocates (structures, arrays, strings) before these three exist and run in CI:

1. **explicit releases in the IR**, with a verifier proving that every owned value is released exactly once on every path, `return`, `break` and traps included;
2. **a debug allocator** that poisons released memory, detects double releases and checks that every allocation is returned when a program ends; the spec already plans such a leak detector for managed mode, and it applies to owned mode too;
3. **the differential test extended to allocations**: both backends must agree on results and on the number of allocations still alive.

Fuzzing, planned for day 10 of the kickoff plan, covers both backends.

## Consequences

- Days 8 and 9 of the plan change: the Vite plugin uses the direct backend and needs no Rust toolchain; driving `cargo` is done by `--backend rust` and by CI only.
- The CI gains a backend job: it installs `wasm-bindgen` 0.2.130 and runs `node scripts/bench-backends.mjs --check`, which fails as soon as the two backends disagree.
- Peras owns more: its optimizer, its allocator (a compact one, as the spec already plans, only linked when a module allocates), SIMD lowering, and debug information and source maps for WebAssembly, which LLVM would otherwise provide.
- The benchmark is small: four kernels, one machine, Node only. It must grow with each optimization, and run in browsers before performance claims are made.

## Follow-up

- **2026-11-04, accumulator recursion elimination** (`crates/perasc-ir/src/opt.rs`). On the direct backend, `fib(32)` goes from 16 ms to 9.4 ms, and `fib(35)` from 68 ms to 39 ms; the Rust backend takes 4.2 ms on `fib(32)`. The IR verifier runs after lowering and after the pass, and the Rust backend compiles the unoptimized IR, so the differential test in CI also covers the pass; an IR interpreter compares both versions on hundreds of inputs in the unit tests. The remaining gap is not explained yet: it stays at 1.5 times on `fib(35)` with V8's optimizing compiler alone, so tiering is not the cause, and checking the base case before the loop, as LLVM does, was measured and brought nothing, so it was not kept.
- **2026-11-04, where Rust's lead came from.** Measured by controlled variants of the WebAssembly function, then confirmed in V8's source (`src/wasm/inlining-tree.h`, `module-decoder-impl.h`):
  - **`fib`: an engine heuristic, not LLVM.** Rust's exact `fib` function, put alone in a module, is as slow as ours (40 ms on `fib(35)`). In the Rust module, V8 inlines `fib` into itself three times; in ours, never. V8 sizes its inlining budget from the share of function bodies under 50 bytes: at half or more, it assumes the toolchain did not inline and allows five times the function's size; at a quarter or less, two times, too little for one level. The Rust module is full of small allocator and glue functions; ours has none. Peras now does this itself, so the gain no longer depends on the engine: **recursion unrolling** (`crates/perasc-ir/src/inline.rs`) copies the body into the remaining self call three levels deep, measured best against two and four, and **exit test rotation** (`opt.rs`, with the backend) checks a loop condition once before the loop and then at the bottom, one branch per turn. `fib(32)` goes from 9.4 ms to 4.0 ms against 4.2 ms for Rust; on `fib(30)` and `fib(35)`, 1.6 and 21.4 ms against 1.7 and 22.6 ms.
  - **`checksum`: a mispredicted branch.** `if (h < 0) h = -h` on a hash takes the branch about one time in two, at random. LLVM writes it without a branch. Peras now rewrites it to `abs`, which the backend writes with `select` (`opt.rs`, integers only: on floats, `-0.0` and NaN would differ). `checksum` goes from 26 ms to 11 ms, Rust's time. LLVM's other changes there (multiplication turned into addition, unrolling by four) were measured at under 5 %.
  - **Still behind: `mandel`**, 15 ms against 13. LLVM reuses `x * x` and `y * y` between the condition and the body, moves what does not change out of the inner loop, replaces `px.toF64()` with a float counter, and unrolls by two. These are general passes (common subexpressions, loop-invariant code motion, induction variables); they are the next ones.
  - Each pass keeps every result, traps included: the IR verifier runs after each one, an IR interpreter compares optimized and unoptimized code on edge inputs in the unit tests, including hand-built IR that reads variables before assigning them, and the differential test in CI compares the direct backend against the Rust backend, which compiles the unoptimized IR.
