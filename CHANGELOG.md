# Changelog

_Version française : [CHANGELOG.fr.md](CHANGELOG.fr.md)_

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/). Before 1.0, any release may break the language.

## 0.0.1 — 2026-11-04

The end of phase 0 of the [kickoff plan](docs/en/kickoff-plan.md): a `.peras` function compiles to WebAssembly and runs in the browser through Vite, and every error shows an excerpt, a cause and a fix, in English and French.

### Language and compiler

- Lexer, parser, name and type checks for functions, `let` and `const`, `struct`, arrays, `if`, `while`, `for` and `for…of`. Every error is reported in one pass, with a stable code, a cause, a fix as a diff, and a `perasc explain` page.
- 46 error codes, each with its page in English and French; the example of every page is checked to produce its code and its fix to remove it.
- `perasc check`, `perasc build` and `perasc explain`, with `--lang en|fr`, `--json` and `--color`.
- Code generation for functions on numbers and booleans:
  - a direct WebAssembly backend, the default, which needs no other tool;
  - a Rust backend, used in CI as a check of the direct one ([decision 0001](docs/en/decisions/0001-webassembly-backend.md)).
- IR optimizations, each measured against Rust and checked by an IR verifier and a differential test:
  - accumulator recursion elimination;
  - recursion unrolling;
  - exit test rotation;
  - branchless integer `abs`;
  - doubling as an addition;
  - loop-invariant code motion.

  On the benchmark kernels, the direct backend is level with the Rust backend.

### Tooling

- `vite-plugin-peras` imports a `.peras` file as a module. It shows errors in Vite's overlay, fails `vite build` on an error, and writes TypeScript declarations next to the source.
- `examples/fib` runs `fib` in the browser.

### Quality gates

Every check blocks the merge:

- formatting;
- lints with no warning;
- unit tests, UI tests and end-to-end tests in headless Chromium;
- `cargo deny` and `pnpm audit` with no known CVE;
- the two backends agreeing;
- translations complete;
- nightly fuzzing of the lexer, the parser and the whole pipeline.

### Known limits

- Strings, arrays, structures, methods and `Option` are checked but not compiled yet: code generation reports them with `E0900`.
- Ownership, `.apeiron` modules and the standard library come in the next phases.
