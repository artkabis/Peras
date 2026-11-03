# Peras — Compiler kickoff plan

_Last updated: 2026-11-03. French translation: [../fr/kickoff-plan.md](../fr/kickoff-plan.md)._

Phase 0 ends when a `.peras` function compiles to WASM, runs in the browser through Vite, and every syntax error prints an excerpt, a cause and a fix in English and French.

## Repository layout

One monorepo holds a Cargo workspace for the compiler and a pnpm workspace for the JS tooling, so a single CI run checks both.

```text
peras/
├─ Cargo.toml              Cargo workspace
├─ rust-toolchain.toml     pinned Rust version
├─ pnpm-workspace.yaml
├─ crates/
│  ├─ perasc-syntax/       lexer, parser, AST, source spans
│  ├─ perasc-diag/         diagnostics, error codes, EN/FR messages
│  ├─ perasc-check/        names, types, ownership, resources, stack budget
│  ├─ perasc-ir/           typed IR shared by both backends
│  ├─ perasc-codegen-rs/   .peras → Rust source
│  ├─ perasc-codegen-js/   .apeiron → JavaScript + source maps
│  ├─ perasc-driver/       pipeline, config, cargo and wasm-bindgen calls
│  └─ perasc/              command-line interface
├─ std/                    standard library written in .peras (raster, math)
├─ packages/
│  ├─ vite-plugin-peras/
│  ├─ vitest-peras/
│  └─ create-peras/
├─ tests/ui/               .peras / .apeiron inputs + expected diagnostics
├─ docs/en/                English documentation (source of truth)
├─ docs/fr/                French documentation
└─ .github/workflows/
```

Each crate depends only on the crates above it in the pipeline. `perasc-diag` is the exception: every crate uses it.

## Technical decisions

The compiler is written in Rust and owns every stage that produces a diagnostic, because verbose errors are a core pillar. Existing crates are used where they only produce output.

| Stage          | Choice                                                                                                                                                                                              | Why                                                                                                             |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Lexer          | `logos`                                                                                                                                                                                             | Fast generated lexer, exact spans for every token                                                               |
| Parser         | Hand-written recursive descent, Pratt parsing for expressions                                                                                                                                       | Full control of error recovery and messages; the syntax diverges from TS (`match`, `struct`, `inout`, `@frame`) |
| AST storage    | Arena + interned identifiers                                                                                                                                                                        | Cheap IDs, no lifetimes leaking through the API                                                                 |
| Checks         | Names and types in one walk (a name already reported becomes an error type, compatible with everything), then ownership → resources → stack budget; checks run only on a file without syntax errors | One mistake gives one error, and no check reports consequences of a broken tree                                 |
| IR             | Typed, desugared IR in `perasc-ir`                                                                                                                                                                  | One input for both backends, easy to snapshot                                                                   |
| Rust backend   | Emit tokens, format with `prettyplease`, build with cargo for `wasm32-unknown-unknown`, then `wasm-bindgen`                                                                                         | Readable generated Rust, rustc as the final safety net                                                          |
| JS backend     | `oxc` AST and codegen                                                                                                                                                                               | Mature JS printer and source maps, no need to write our own                                                     |
| Incrementality | Queries designed to be `salsa`-friendly from day one, `salsa` adopted at the `--watch` milestone                                                                                                    | Avoids a rewrite when the LSP arrives                                                                           |
| Safety         | `#![forbid(unsafe_code)]` in every compiler crate                                                                                                                                                   | The compiler holds itself to the rule it enforces                                                               |

This replaces the “extended oxc parser” idea from an earlier draft of the [spec](spec.md): oxc stays for JS output, the parser is ours.

## Diagnostics engine

`perasc-diag` is built before the parser, so the very first syntax error already has the final format.

- **One data type**: a `Diagnostic` holds a stable code, a severity (always error), labelled spans, a cause, the value history, a suggested fix as a diff, and a link to `perasc explain`.
- **Rendering**: our own renderer, in the exact format of the [spec](spec.md#diagnostic-format): excerpt with several labels, cause, fix as a diff, `perasc explain`. `ariadne` was the first choice, but it hard-codes English words ("Help", "Note") and cannot show a fix as a diff. Invisible and bidirectional characters in an excerpt are shown as `<U+202E>`, so an error never reorders the terminal.
- **JSON**: the same `Diagnostic` serialized with `serde` for `--json`, the Vite overlay, CI and the future LSP.
- **Messages in two languages**: message texts live in Project Fluent files, `messages/en.ftl` and `messages/fr.ftl`. Code only references message IDs, never raw strings.
- **Error code registry**: one file lists every code (`E0001`…), its message ID and its doc pages in `docs/en/errors/` and `docs/fr/errors/`.
- **All errors in one pass**: the parser recovers at statement boundaries, and each check pass reports every error it finds, capped at 50 per file.

## Quality gates

Every pull request must pass all of these checks; any failure blocks the merge.

| Check             | Tool                                                            | Blocks on                          |
| ----------------- | --------------------------------------------------------------- | ---------------------------------- |
| Formatting        | `cargo fmt --check`, Prettier                                   | any diff                           |
| Lints             | `cargo clippy -- -D warnings`, ESLint                           | any warning                        |
| Unit and UI tests | `cargo test`, `insta` snapshots on `tests/ui/`                  | any failure or unreviewed snapshot |
| JS tooling tests  | Vitest for the Vite plugin                                      | any failure                        |
| End-to-end        | build the demo with Vite, run it headless                       | build error or wrong output        |
| Rust dependencies | `cargo audit`, `cargo deny`                                     | any known CVE, banned licence      |
| JS dependencies   | `pnpm audit`                                                    | any known CVE                      |
| Translations      | script comparing `en.ftl` and `fr.ftl`, `docs/en` and `docs/fr` | a key or page missing in French    |
| Fuzzing           | `cargo-fuzz` on lexer and parser, nightly run                   | any crash                          |

Dependencies stay on their latest versions; Renovate opens the update PRs, and the gates above decide.

## First two weeks

Ten working days, each closing on something that runs or is tested.

1. **Workspace and CI**: Cargo and pnpm workspaces, pinned toolchain, every quality gate wired up and green on an empty project.
2. **Diagnostics**: `Diagnostic` type, code registry, terminal rendering, Fluent files in English and French, `--json` output.
3. **Lexer**: `logos` tokens with spans, an error for every invalid character, first snapshot tests.
4. **Parser, part 1**: functions, `let` and `const`, `i32` and `f64` literals, arithmetic with Pratt parsing.
5. **Parser, part 2**: `struct`, `Array`, `if`, `for`, `return`, recovery at statement boundaries, 30 UI tests.
6. **Names and types**: scopes, no shadowing, unused-variable error, no implicit numeric conversion.
7. **IR and Rust backend**: lower the subset to IR, emit formatted Rust, snapshot the output.
8. **Driver**: `perasc build` calls cargo for `wasm32-unknown-unknown` and `wasm-bindgen`, then reports any rustc error as a compiler bug.
9. **Vite plugin**: `vite-plugin-peras` loads a `.peras` file as a WASM module; `fib` runs in the browser.
10. **Hardening and release**: nightly fuzzing, docs for every code issued so far in English then French, tag `v0.0.1`.

Exit criteria: `fib` runs in the browser, all gates are green, and each syntax error shows excerpt, cause and fix in both languages.

## Working rules

- **English first, then French**: English is the source of truth for code comments, docs, messages and commit messages. Every user-facing text lands in English and French in the same pull request; the translation gate enforces it.
- **Error codes are permanent**: a code is never reused or renumbered. A retired code keeps its doc page, marked as retired.
- **Every new error ships complete**: code, English and French messages, a suggested fix, a UI test and a `perasc explain` page.
- **Conventional Commits** (`feat:`, `fix:`, `docs:`…) to generate the changelog.
- **Versioning**: semver, `0.x` until the language is stable; any breaking change is listed in the changelog.
- **Releases**: prebuilt `perasc` binaries per platform, published to npm as optional dependencies, the way esbuild and oxc ship.

## Open decisions

- [ ] Licence: MIT, Apache-2.0, or both as in the Rust ecosystem?
- [ ] Minimum supported Rust version for contributors.
- [ ] Hosting: GitHub organisation name, once npm, crates.io and the domain are reserved.
- [ ] Should `salsa` be adopted earlier than the `--watch` milestone?
