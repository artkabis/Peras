# 0001: How `.peras` modules become WebAssembly

_French translation: [../../fr/decisions/0001-webassembly-backend.md](../../fr/decisions/0001-webassembly-backend.md)._

**Status:** proposed, 2026-11-04. The kickoff plan keeps its Rust backend until this decision is accepted.

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

## Decision (proposed)

1. **The direct backend becomes the default**, for development and production: no tool to install beyond `perasc`, builds in milliseconds, the smallest modules.
2. **Peras gets its own optimization passes on the IR**, chosen and measured with `bench/backends`: first the accumulator transformation that explains `fib`, then the cause of the `checksum` gap once found. The IR knows what LLVM must prove, notably that `inout` arguments never alias.
3. **The Rust backend stays, as a secondary backend** behind `--backend rust`: a reference for performance until the direct backend matches it, and the second half of a differential test, where both backends must agree on every program and input. This replaces the plan's rule that `rustc` must never reject a program `perasc` accepted.
4. **Every generated module is validated** with `wasmparser` in the tests.

## Consequences

- Days 8 and 9 of the plan change: the Vite plugin uses the direct backend and needs no Rust toolchain; driving `cargo` becomes optional.
- Peras owns more: its optimizer, its allocator (a compact one, as the spec already plans, only linked when a module allocates), SIMD lowering, and debug information and source maps for WebAssembly, which LLVM would otherwise provide.
- The benchmark is small: four kernels, one machine, Node only. It must grow with each optimization, and run in browsers before performance claims are made.
