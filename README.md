# Peras

_Version française : [README.fr.md](README.fr.md)_

Peras is a web language with TypeScript syntax and Rust-grade rigour. `.peras` modules compile to WebAssembly with exact, garbage-collector-free memory; `.apeiron` modules compile to JavaScript, disciplined by the compiler.

> Peras (πέρας) is Greek for _the limit_. For the Pythagoreans, everything good arises from limit imposed on the unlimited (_apeiron_).

**Status:** version 0.0.1, the end of phase 0 of the kickoff plan ([changelog](CHANGELOG.md)). `perasc` checks the syntax, names and types of a source, and compiles functions on numbers and booleans to WebAssembly, without a Rust toolchain; `vite-plugin-peras` runs them in the browser. Structures, arrays, ownership and `.apeiron` modules come next.

## Pillars

1. **Rasterization**: typed framebuffers shared with the Canvas without copying.
2. **Memory**: single ownership, deterministic release, no allocation inside a render loop.
3. **Approachable syntax**: TypeScript developers are productive within a day.

Every diagnostic blocks the build and explains the cause, the value's history and the fix, in English or French.

## Documentation

- [Language spec](docs/en/spec.md)
- [Compiler kickoff plan](docs/en/kickoff-plan.md)

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

To be decided (MIT, Apache-2.0, or both). The manifests use `MIT OR Apache-2.0` as a placeholder until then.
