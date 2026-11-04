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
