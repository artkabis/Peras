# 0021: Phases 4 and 5, and the gaps of the language

_French translation: [../../fr/decisions/0021-phases-4-and-5.md](../../fr/decisions/0021-phases-4-and-5.md)._

_Status: accepted on 2026-11-12 by the owner. A plan: each step lands with its tests, and this page says what landed._

## Context

The [roadmap](../reference/roadmap.md) puts Peras in phase 3, the standard library. The owner asks for phases 4 and 5 next, with the gaps the findings keep open:

- **Phase 4, managed mode.** It is the half of the design that talks to the browser. Its next step, measured by [decision 0011](0011-what-to-build-next.md), is structures across `.apeiron` and `.peras`.
- **Phase 5, modules.** A program in several `.peras` files is not possible yet.

What a step changes in the language is written in the guide and the status the day it lands.

## The order

Each step is small enough to land alone, and the most used come first.

1. **Structures and enumerations across `.apeiron` and `.peras`.**
   - Today an `.apeiron` module can call a `.peras` function only with numbers, booleans, strings, and arrays, options, results and maps of them. The checker's `portable` refuses the rest.
   - Both modes already give a structure to JavaScript as a plain object, and an enumeration as `{ kind, values }`.
   - The step: a managed module imports the structures and enumerations a `.peras` module exports, as it imports its functions: `import { area, Rect } from "./shapes.peras"`. It can name them, make them, read them, and pass them across.
2. **Small gaps, each checked but not compiled:**
   - `%` on floats (F304);
   - `==` on arrays of strings or of arrays (F305);
   - `reduce` with an accumulator that holds a string or an array (F307);
   - `copy()` on a structure (F102).
3. **`import` and `export` between `.peras` modules.**
   - Several files compiled into one WebAssembly module: names resolved across files, each file checked against what the others export.
   - The design is written first, as its own decision, before code: files, cycles, names, what a build produces. Written: [decision 0022](0022-owned-modules.md).
4. **Medium gaps:**
   - module-level `let` (F306);
   - `u8`, `u16`, `u32`, `u64` compiled (F302);
   - a field moved out of an owned structure (F214);
   - a named function used as a value (F308).

Later, each with its own decision:

- types that contain themselves and `Rc<T>` (F310);
- `let … else` and an arm that leaves a loop (F101, F105), which are questions of design more than of code;
- the browser APIs phase 4 still lists;
- `extern` from `.d.ts`, and packages.

## Criteria of each step

A step lands only with all of these:

- **Tests in Rust, of the checker or of the IR.** These include the cases that must still be refused.
- **End-to-end tests on both allocators, where the step reaches the browser.** They compare with JavaScript, and nothing may be left alive.
- **The differential test, where the step touches what the optimizations of decisions 0016 to 0018 see.** It runs once with 2,000 seeds, and once with the change broken on purpose (M015).
- **Every check of `CLAUDE.md`,** e2e on a freshly built release binary included (M018).
- **The guide, the status and the roadmap updated,** in English and in French.

## Record

_Each step is recorded here as it lands._

**Step 1, structures and enumerations across `.apeiron` and `.peras`: landed.**

- **What changed.**
  - The checker gives a managed module the structures and enumerations of the owned modules it imports, by their index there. It takes in, once and under its name, each one an import names or an imported function's types reach, its fields and variants translated.
  - A generic structure does not cross.
  - A type of the same name as one the managed module declares is reported (`E0244`, `clash`).
  - The JavaScript a managed module compiles to imports only the functions.
- **Tests.**
  - The checker's UI tests: a structure imported and used, a generic one refused, a name that clashes.
  - An end-to-end test, on both allocators: a nested structure with an array of strings, an enumeration each way and an array of structures, the result compared and nothing left alive.
  - The fields of a structure taken in out of order, on purpose, fail it.

**Step 2, small gaps: landed.**

- **`%` on floats (F304): landed.** `f64_rem`, a function of the runtime written after musl's `fmod`; an `f32` computed as an `f64`. Tested against Rust's `%` on 2,000,000 random pairs and the edges of IEEE 754, against JavaScript's end to end, and in the differential test. A module that calls the runtime without any buffer now imports it (F218).
- **`==` on arrays of strings and of arrays (F305): landed.** `buf_equal`, a function of the runtime, given each column's shape: its kind of element, and for a string or an array, the shape of what it holds. It compares lengths, then elements, going down into each; floats compare as numbers. Tested end to end against JavaScript on both allocators, and in the differential test with 2,000 seeds; floats compared as bytes, on purpose, fail both. Arrays of maps, sets, or options that hold memory are still reported.
- **`reduce` with a string or an array as its accumulator (F307): landed.** The accumulator owns its strings and arrays, copied when the start or the body only lends them; each turn computes the new value, then releases the previous one. Tested end to end against JavaScript on both allocators, and in the differential test with 2,000 seeds; the previous value released too early fails the differential test, and never released, the release verifier.
- **`copy()` on a structure (F102): landed.** Each string, array, map and set of the structure copied by the runtime with the buffers it owns; in managed mode, `structuredClone`. A structure holding a resource of the browser, a value of JavaScript or a function has none (`E0215`). Tested end to end on both allocators, in the differential test with 2,000 seeds, and in managed mode by Node; buffers shared rather than copied, on purpose, are refused by the release verifier.
- **Found on the way.** The vectorizer summed its counts of reads with wrapping (F219), and an arrow function of a managed module that makes a structure was emitted as a block; both fixed, each with a test that fails without the fix.

**Step 3, `import` and `export` between `.peras` modules: landed** for functions, structures and enumerations that are not generic, as [decision 0022](0022-owned-modules.md) designed and records.
