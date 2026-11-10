# Peras

_Version française : [README.fr.md](README.fr.md)_

Peras is a web language with TypeScript syntax and Rust-grade rigour. `.peras` modules compile to WebAssembly with exact, garbage-collector-free memory; `.apeiron` modules compile to JavaScript, disciplined by the compiler.

> Peras (πέρας) is Greek for _the limit_. For the Pythagoreans, everything good arises from limit imposed on the unlimited (_apeiron_).

**Status:** version 0.0.1, in phase 3 of the [roadmap](docs/en/reference/roadmap.md), the standard library ([changelog](CHANGELOG.md), [what is compiled today](docs/en/reference/status.md)). `perasc` checks the syntax, names, types and ownership of a source, and compiles it to WebAssembly without a Rust toolchain: numbers, structures, enumerations and `match`, options and results, generics, methods and interfaces, arrays and strings with their methods, arrow functions for `map`, `filter` and the others, template literals, and `Map` and `Set`. Every block of memory is released by the compiler, checked by a verifier, with a debug allocator in development and TLSF in production. `vite-plugin-peras` runs modules in the browser. Threads over shared memory ([decision 0007](docs/en/decisions/0007-threads-over-shared-memory.md)) and `.apeiron` modules come next.

**Speed**, measured in Chromium against JavaScript written for speed ([benchmark](bench/README.md#browser-benchmark-benchbrowser)): 5.4 times faster on 64-bit integers, 1.6 to 2.4 times on numbers in arrays, sorting and text, the same speed on structures and floating-point arithmetic alone, and slower on very short calls across the boundary. Against JavaScript as usually written, the gaps are larger; both are published. Against Rust and AssemblyScript as usually written, Peras runs at the speed of Rust, ahead of it on four kernels where its compiler does more, and faster than AssemblyScript wherever arrays or allocation are involved.

## Pillars

1. **Rasterization**: typed framebuffers shared with the Canvas without copying.
2. **Memory**: single ownership, deterministic release, no allocation inside a render loop.
3. **Approachable syntax**: TypeScript developers are productive within a day.

Every diagnostic blocks the build and explains the cause, the value's history and the fix, in English or French.

## Documentation

- [Guide](docs/en/guide/introduction.md), from the first program to JavaScript interop
- [Reference](docs/en/reference/status.md): status, types, operators, methods, keywords, CLI, [roadmap](docs/en/reference/roadmap.md)
- [Error codes](docs/en/errors/index.md), each with its cause and fix
- Decisions: [0001 WebAssembly backend](docs/en/decisions/0001-webassembly-backend.md), [0002 TLSF allocator](docs/en/decisions/0002-tlsf-allocator.md), [0003 a runtime proved at compile time](docs/en/decisions/0003-runtime-proved-at-compile-time.md), [0004 strings without allocation](docs/en/decisions/0004-strings-without-allocation.md), [0005 maps and sets](docs/en/decisions/0005-maps-and-sets.md), [0006 licensing](docs/en/decisions/0006-licensing.md), [0007 threads over shared memory](docs/en/decisions/0007-threads-over-shared-memory.md)
- [Benchmarks](bench/README.md), what they measure and what they do not
- [Language spec](docs/en/spec.md), [architecture](docs/en/architecture.md), [compiler kickoff plan](docs/en/kickoff-plan.md)

## Repository layout

| Path                         | Contents                                            |
| ---------------------------- | --------------------------------------------------- |
| `crates/perasc-syntax`       | Lexer, parser, AST                                  |
| `crates/perasc-diag`         | Diagnostics, error codes, EN/FR messages            |
| `crates/perasc-check`        | Names, types, ownership, resources, stack budget    |
| `crates/perasc-ir`           | Typed IR shared by both backends                    |
| `crates/perasc-codegen-rs`   | `.peras` → Rust → WASM                              |
| `crates/perasc-codegen-wasm` | `.peras` → WASM, directly, without a Rust toolchain |
| `crates/perasc-codegen-js`   | `.apeiron` → JavaScript                             |
| `crates/perasc-driver`       | Pipeline and build orchestration                    |
| `crates/perasc-runtime`      | WebAssembly runtime: allocators, strings, arrays    |
| `crates/perasc`              | Command-line interface                              |
| `packages/vite-plugin-peras` | Vite plugin                                         |
| `packages/vitest-peras`      | Vitest adapter                                      |
| `packages/create-peras`      | Project scaffolding                                 |
| `examples/fib`               | `fib` in Peras, run in the browser through Vite     |
| `std/`                       | Standard library, written in Peras                  |
| `tests/ui/`                  | Source files and their expected diagnostics         |
| `docs/en`, `docs/fr`         | Documentation, English first                        |

## Getting started

Requirements: [rustup](https://rustup.rs), Node 26 LTS (26.13.0+), pnpm 12+.

```sh
# Rust: rustup installs the toolchain pinned in rust-toolchain.toml
cargo test --workspace
cargo run -p perasc -- --version

# Check sources: every error with excerpt, cause and fix (--lang en|fr, --json)
cargo run -p perasc -- check crates/perasc/tests/fixtures/demo.peras
cargo run -p perasc -- explain E0007

# JS: once, installs the newest mature (>24 h old) stable dev dependencies, audits them and runs every check
pnpm run bootstrap
```

Before the first push, commit the `pnpm-lock.yaml` created by the bootstrap: CI installs it frozen.

## Run Peras in the browser

```sh
cargo build --release -p perasc
pnpm --filter peras-example-fib dev
```

`examples/fib/src/main.ts` imports `fib` from `fib.peras` like any module. `vite-plugin-peras` compiles it with `perasc`'s direct backend: in development the WebAssembly is inlined in the module, and `vite build` emits it as a hashed asset, compiled while it downloads. An error in the source shows in Vite's overlay with its excerpt, cause and fix, and fails `vite build`. The plugin also writes `fib.d.peras.ts` next to the source, so TypeScript knows the real signatures (with `allowArbitraryExtensions`).

```ts
// vite.config.ts
import peras from "vite-plugin-peras";

export default { plugins: [peras({ perasc: "path/to/perasc", lang: "en" })] };
```

`pnpm run e2e` builds the example, opens it in a headless Chromium on the dev server and on the production build, and checks the overlay shows errors. It needs `target/release/perasc` and a Chromium: `pnpm exec playwright install chromium`, or `PERAS_CHROMIUM` set to an installed one.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). In short: English first, then French in the same pull request; no warning, no `unsafe`, no known CVE.

## Licence

Peras is free software under the [GNU Affero General Public License, version 3 or later](LICENSE), with the [Peras Runtime Exception](RUNTIME-EXCEPTION.md): the programs you compile with Peras are yours, under the terms you choose, closed ones included; Peras itself, modified or not, stays under the AGPL, as a service too. The code examples of the documentation and of `examples/` are under 0BSD. The memory leak analyzer and the VS Code extension are separate, proprietary tools. Why: [decision 0006](docs/en/decisions/0006-licensing.md).

Copyright © 2026 Artkabis and the Peras contributors.
