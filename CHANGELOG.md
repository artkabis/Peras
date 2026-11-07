# Changelog

_Version française : [CHANGELOG.fr.md](CHANGELOG.fr.md)_

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/). Before 1.0, any release may break the language.

## Unreleased

### Language and compiler

- **Structures** compile, as values: each one is replaced by its scalar fields, nested structures flattened, so it never touches the heap.
  - A structure parameter becomes one parameter per field.
  - A function returning a structure returns one WebAssembly result per field, and a tuple in the Rust backend.
  - Fields are read and written, compound assignments included; a structure is copied by assignment; `p = Point(p.y, p.x)` swaps.
  - Calls returning a structure run first, into temporaries, without changing the order of evaluation: on the right of `&&` and `||` only when it would run, and in a loop condition at every turn.
  - Exported functions take and return JavaScript objects, declared as TypeScript interfaces.
- **Arrays of strings and of arrays** compile, and structures holding them in arrays: a buffer can own buffers, which the runtime releases and copies with it, at any depth. JavaScript passes and receives them as nested arrays and objects.
- E0204 tells the value's history: where it received its value, where it moved, where it is used again.
- **Arrays of structures and options** compile, as columns: one buffer per scalar of an element, so `points[i].x` and a loop over one field touch one buffer. Indexing, `points[i].x = v`, `push`, `pop`, `copy`, `==` and `for…of` work column by column; JavaScript sees arrays of objects.
- **`inout`** works on every type: the function gives its numbers back as extra results, written back into the caller's variables; arrays and strings change in place. From JavaScript, a structure passed `inout` is an object whose fields are updated. A variable passed `inout` cannot be read elsewhere in the same statement (`E0207`), and different fields of one structure no longer count as aliases.
- `?` now releases the arrays and strings a function holds before it returns `None`; they leaked before.
- **Strings** compile, in UTF-8: literals, `+`, `+=`, `==`, `!=`, `length` in bytes and `copy()`. A literal is stored once, as a passive data segment, and copied into a new buffer with `memory.init`; `+=` appends in place. JavaScript passes and receives strings, encoded and decoded as UTF-8. `==` also compares arrays, element by element.
- **Arrays** compile, of numbers and booleans, on the heap: literals, `length`, indexing and assignment through an index, `push`, `pop`, `copy`, `for…of`.
  - Every index is checked; one out of bounds stops the program with a `RangeError`, in English or French, before memory is touched.
  - An array is a 12-byte header, which keeps its address, and its elements; `push` doubles the room when it runs out. The debug allocator holds both blocks, so every memory bug stays caught.
  - JavaScript passes and receives copies, as `number[]`, `bigint[]` or `boolean[]`; an `inout` parameter is copied back.
- **Ownership** is checked: a value on the heap has one owner, moves on assignment, `return` and `sink`, and is borrowed otherwise.
  - Four new errors: a value used after it moved ([E0204](docs/en/errors/E0204.md)), `&` missing or extra on an `inout` argument ([E0206](docs/en/errors/E0206.md)), two arguments reaching one variable that one of them changes, or an array changed while `for…of` goes through it ([E0207](docs/en/errors/E0207.md)), moving what is only borrowed ([E0209](docs/en/errors/E0209.md)). Each fix adds `.copy()`, `&` or `sink` where it applies.
  - Lowering inserts every release: at the end of a scope, on reassignment, before `return`, `break` and `continue`, and where paths meet after a move on one of them. The release verifier proves each one; the test programs leave no block alive, in the interpreter and as WebAssembly on the debug allocator.
- **Options** compile, also without the heap: an `Option<T>` is a `bool` followed by the scalars of `T`, any compiled `T`, structures included.
  - `Some`, `None`, `isSome`, `isNone`, `unwrapOr`, and `?`, which returns `None` early from a function returning an option.
  - `checkedAdd`, `checkedSub` and `checkedMul` on `i32` and `i64` give `None` on overflow; they are tested against Rust's own on the values where overflow starts and stops.
  - `None` and `[]` take the type their context expects.
  - JavaScript sees the value or `undefined`, declared as `T | undefined`.
- The JavaScript loader prefixes every name it declares with `$`, which a Peras name cannot start with: a function or a parameter named `r`, `url` or `runtime` no longer breaks the module.
- The IR verifies ownership: `owned` and `borrowed` handles, explicit releases, and a verifier proving that every block is released exactly once on every path. The debug allocator is linked into modules that use the heap; a memory bug throws a `PerasMemoryError` naming both sites, in English or French. Nothing in the language allocates yet.

### Benchmark

- `overflows` exercises options and checked arithmetic; both backends agree with the JavaScript reference.
- `cmandel` computes the Mandelbrot set with `struct Complex`. Both backends agree with the JavaScript reference. The direct backend takes 15 ms against 13 ms for plain `f64` code and for Rust; JavaScript, which allocates an object per complex number, takes 49 ms. The optimizer does not touch functions with several results yet.

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
