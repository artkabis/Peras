# Peras

_Version française : [README.fr.md](README.fr.md)_

Peras is a web language with TypeScript syntax and Rust-grade rigour. `.peras` modules compile to WebAssembly with exact, garbage-collector-free memory; `.apeiron` modules compile to JavaScript, disciplined by the compiler.

![Before and after, zoomed 8 and 64 times into the same corner of the garden of Apeiron. Left, the stored image, an AVIF of 9.5 KB, enlarged: blocks of colour, then a flat green. Right, the Peras program of 5.7 KB, drawing the same window at that zoom: sharp flowers, their petals and hearts, and smaller ones between them](docs/assets/garden-before-after.webp)

_**Before, after.** The same corner of the garden of Apeiron, zoomed 8 then 64 times. Left, what a stored image gives: the smallest AVIF of the whole scene that looks right at full size, 9.5 KB, enlarged; zoomed 64 times, 16 pixels of it fill the frame, and nothing is left. Right, what the Peras program gives, 5.7 KB: it draws the window at that zoom, and the closer you look, the more flowers it makes. Even the lossless PNG, 471 KB, has only 16 pixels across at ×64; to match the program there, a stored image would need some 41.5 MB ([decision 0013](docs/en/decisions/0013-garden-of-apeiron.md), measured under a protocol registered before any measure; `node scripts/garden-showcase.mjs` makes this image again, its numbers computed from the files)._

**[Zoom into it yourself in the playground](https://artkabis.github.io/Peras/playground/)**: the stored image and the program side by side, down to 65,536 times ([source](examples/garden)).

![The garden of Apeiron at three zooms: the whole landscape, a sky at dawn, three ranges of mountains and a meadow; the meadow zoomed 8 times, full of flowers; zoomed 64 times, single flowers with their petals and hearts, and smaller ones between them](docs/assets/garden.webp)

_The garden of Apeiron, zoomed 1, 8 and 64 times: a scene drawn pixel by pixel from a few rules, that gains detail as it is zoomed ([decision 0013](docs/en/decisions/0013-garden-of-apeiron.md)). Its hypotheses were registered before any measure, and its three versions, TypeScript, Peras and Rust, give the same pixels, bit for bit. Four held: the 6 KB program is smaller than the smallest AVIF of its image at SSIM 0.95, 10 KB; zoomed 64 times, a stored image would need 4,452 times more; every lossy format scores higher seen from afar; the overflow checks cost 2 % of a frame. One was refuted: Peras renders it 1.19 times as fast as TypeScript, and 1.11 times as fast as Rust, short of the 1.3 times registered. And the same program in TypeScript ships five times smaller than Peras's, whose runtime weighs ([results](bench/garden/RESULTS.md))._

![The same animation of 1,500,000 particles in three versions, side by side: TypeScript with immutable objects at 8 frames a second, its memory climbing to over 1 GB and falling back at each collection; TypeScript with typed arrays by hand at 47 frames a second; Peras at 58 frames a second, its memory flat](docs/assets/demo.gif)

_The same animation, 1.5 million particles, in Chromium on 4 cores ([results](bench/demo/RESULTS.md), `node scripts/demo.mjs` to run it again). Left, TypeScript as it is often written, a new object per particle at each frame: 8 frames a second, the heap climbing past 1 GB and falling at each collection. Middle, TypeScript written for speed by hand, typed arrays: 47 frames a second, flat. Right, Peras compiled to WebAssembly: 58 frames a second, the same memory as the expert's, flat. Typed arrays bring TypeScript most of the way; Peras gets there from code written plainly, and the rest of the way. Each panel draws its own frame rate and memory, on one scale._

**Where Peras falls short, measured.** On ingesting data, a sales dashboard computed from a CSV of up to 4 million rows, Peras written plainly is slower than TypeScript written plainly, 0.85 to 0.88 times its speed, ten times slower than TypeScript optimized by hand, holds more memory and ships fifteen times more code; five of seven hypotheses registered before the measure are refuted ([decision 0014](docs/en/decisions/0014-data-at-scale.md), [results](bench/data/run-1/RESULTS.md)). The study names why, integer parsing and maps keyed by strings, and what the compiler does next. Every bug, friction and error of method found along the way is kept in [the findings](docs/en/findings.md).

> Peras (πέρας) is Greek for _the limit_. For the Pythagoreans, everything good arises from limit imposed on the unlimited (_apeiron_).

**Status:** version 0.0.1, in phase 3 of the [roadmap](docs/en/reference/roadmap.md), the standard library ([changelog](CHANGELOG.md), [what is compiled today](docs/en/reference/status.md)). `perasc` checks the syntax, names, types and ownership of a source, and compiles it to WebAssembly without a Rust toolchain: numbers, structures, enumerations and `match`, options and results, generics, methods and interfaces, arrays and strings with their methods, arrow functions for `map`, `filter` and the others, template literals, `Map` and `Set`, and `parallel for` loops on every core with `--threads on`. Every block of memory is released by the compiler, checked by a verifier, with a debug allocator in development and TLSF in production. `vite-plugin-peras` runs modules in the browser. `.apeiron` modules compile to JavaScript, with `async` and `await`, and call `.peras` modules ([managed modules](docs/en/guide/managed.md)); the typed browser comes next.

**Speed**, measured in Chromium against JavaScript written for speed ([benchmark](bench/README.md#browser-benchmark-benchbrowser)): 5.4 times faster on 64-bit integers, 1.6 to 2.4 times on numbers in arrays, sorting and text, the same speed on structures and floating-point arithmetic alone, and slower on very short calls across the boundary. Against JavaScript as usually written, the gaps are larger; both are published. Against Rust and AssemblyScript as usually written, Peras runs at the speed of Rust, ahead of it on four kernels where its compiler does more, and faster than AssemblyScript wherever arrays or allocation are involved.

## Pillars

1. **Rasterization**: typed framebuffers shared with the Canvas without copying.
2. **Memory**: single ownership, deterministic release, no allocation inside a render loop.
3. **Approachable syntax**: TypeScript developers are productive within a day.

Every diagnostic blocks the build and explains the cause, the value's history and the fix, in English or French.

## Documentation

The site, with search, in English and French: **<https://artkabis.github.io/Peras/>**, built from `docs/` on every push to `main`. For a first decision: [Peras for decision-makers](docs/en/guide/overview.md).

- [Guide](docs/en/guide/introduction.md), from the first program to JavaScript interop
- [Reference](docs/en/reference/status.md): status, types, operators, methods, keywords, CLI, [roadmap](docs/en/reference/roadmap.md)
- [Error codes](docs/en/errors/index.md), each with its cause and fix
- Decisions: [0001 WebAssembly backend](docs/en/decisions/0001-webassembly-backend.md), [0002 TLSF allocator](docs/en/decisions/0002-tlsf-allocator.md), [0003 a runtime proved at compile time](docs/en/decisions/0003-runtime-proved-at-compile-time.md), [0004 strings without allocation](docs/en/decisions/0004-strings-without-allocation.md), [0005 maps and sets](docs/en/decisions/0005-maps-and-sets.md), [0006 licensing](docs/en/decisions/0006-licensing.md), [0007 threads over shared memory](docs/en/decisions/0007-threads-over-shared-memory.md), [0008 managed mode](docs/en/decisions/0008-managed-mode.md), [0009 the browser and resources](docs/en/decisions/0009-browser-and-resources.md), [0010 calling JavaScript](docs/en/decisions/0010-calling-javascript.md), [0011 what to build next, decided by measurement](docs/en/decisions/0011-what-to-build-next.md), [0012 the cost on the machine, measured with every step](docs/en/decisions/0012-cost-on-the-machine.md), [0013 the garden of Apeiron, a procedural scene measured](docs/en/decisions/0013-garden-of-apeiron.md), [0014 data at scale, aggregated in the browser (protocol)](docs/en/decisions/0014-data-at-scale.md); [the research behind Peras](docs/en/research.md), each choice with the published work it rests on
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
