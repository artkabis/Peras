# Peras

*Version française : [README.fr.md](README.fr.md)*

Peras is a web language with TypeScript syntax and Rust-grade rigour. `.peras` modules compile to WebAssembly with exact, garbage-collector-free memory; `.apeiron` modules compile to JavaScript, disciplined by the compiler.

> Peras (πέρας) is Greek for *the limit*. For the Pythagoreans, everything good arises from limit imposed on the unlimited (*apeiron*).

**Status:** day 1 of the kickoff plan. The repository builds, lints and tests, but the compiler does not compile anything yet.

## Pillars

1. **Rasterization**: typed framebuffers shared with the Canvas without copying.
2. **Memory**: single ownership, deterministic release, no allocation inside a render loop.
3. **Approachable syntax**: TypeScript developers are productive within a day.

Every diagnostic blocks the build and explains the cause, the value's history and the fix, in English or French.

## Documentation

- [Language spec](docs/en/spec.md)
- [Compiler kickoff plan](docs/en/kickoff-plan.md)

## Repository layout

| Path | Contents |
| --- | --- |
| `crates/perasc-syntax` | Lexer, parser, AST |
| `crates/perasc-diag` | Diagnostics, error codes, EN/FR messages |
| `crates/perasc-check` | Names, types, ownership, resources, stack budget |
| `crates/perasc-ir` | Typed IR shared by both backends |
| `crates/perasc-codegen-rs` | `.peras` → Rust → WASM |
| `crates/perasc-codegen-js` | `.apeiron` → JavaScript |
| `crates/perasc-driver` | Pipeline and build orchestration |
| `crates/perasc` | Command-line interface |
| `packages/vite-plugin-peras` | Vite plugin |
| `packages/vitest-peras` | Vitest adapter |
| `packages/create-peras` | Project scaffolding |
| `std/` | Standard library, written in Peras |
| `tests/ui/` | Source files and their expected diagnostics |
| `docs/en`, `docs/fr` | Documentation, English first |

## Getting started

Requirements: [rustup](https://rustup.rs), Node 24+, pnpm 10+.

```sh
# Rust: rustup installs the toolchain pinned in rust-toolchain.toml
cargo test --workspace
cargo run -p perasc -- --version

# JS: once, installs the latest dev dependencies, audits them and runs every check
pnpm run bootstrap
```

Before the first push, commit the `pnpm-lock.yaml` created by the bootstrap: CI installs it frozen.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). In short: English first, then French in the same pull request; no warning, no `unsafe`, no known CVE.

## Licence

To be decided (MIT, Apache-2.0, or both). The manifests use `MIT OR Apache-2.0` as a placeholder until then.
