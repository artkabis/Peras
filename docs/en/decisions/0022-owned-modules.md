# 0022: Owned modules that import each other

_French translation: [../../fr/decisions/0022-owned-modules.md](../../fr/decisions/0022-owned-modules.md)._

_Status: proposed on 2026-11-12, with its criteria registered before any code; step 3 of [decision 0021](0021-phases-4-and-5.md)._

## Context

An owned module (`.peras`) is one file. Every function, structure and enumeration a program needs is in that file. A managed module (`.apeiron`) can already import from owned modules ([decision 0008](0008-managed-mode.md), [decision 0021](0021-phases-4-and-5.md), step 1). An owned module cannot import anything: `import` in a `.peras` file is refused as "managed only".

An application of a fair size does not fit in one file. Phase 5 of the [roadmap](../reference/roadmap.md) lists `import` and `export` between Peras modules first. An application of reference, written in Peras to see whether a fairly standard application can be built and deployed with it, needs it too.

## What others have done

- **ECMAScript modules.** `import { a } from "./b.js"` names a file by a relative path and only takes what the file exports. A cycle is allowed and resolved at run time, which lets a module read a binding before it is set (a `ReferenceError` in the temporal dead zone).
- **Rust.** A crate is the unit of compilation, and its modules are files within it. Cycles between modules are allowed, because the crate is checked as a whole. Cycles between crates are refused.
- **Go.** A package is a directory. Import cycles are refused by the compiler, with the path of the cycle printed. The rule is defended as what keeps builds fast and dependencies readable.
- **Zig.** `@import("file.zig")` makes the file a structure of declarations. Everything a build reaches is compiled into one unit, and only what is reached is analysed.

## Decision

1. **The syntax is the one managed modules already use.** It is `import { area, Size } from "./shapes.peras";` at the top of a file. The path is relative to the importing file and names a `.peras` file, extension included. An owned module cannot import a `.apeiron` file: managed code compiles to JavaScript, which an owned module cannot call.
2. **`export` keeps its meaning and gains one.** An exported function, structure or enumeration can be imported by another owned module. What reaches JavaScript is what the file being built exports, as today. A file built on its own keeps exactly the interface it has now, even if another file imports it.
3. **One build, one WebAssembly module.** `perasc build main.peras` follows the imports. Every file reached is compiled into the same module, with one memory, one allocator and one runtime. A value passed from one file to another crosses no boundary and is not copied. Moves and borrows are checked across files as they are within one.
4. **Each file is checked on its own, against the signatures the files it imports export.**
   - Its private names stay its own: two files may each have a function `helper`.
   - The lowered modules are linked into one before the optimizations of decisions 0016 to 0018 run. Those optimizations, inlining included, therefore see across files.
5. **Cycles are refused** (new error `E0250`), with the path of the cycle: `a.peras → b.peras → a.peras`.
   - This is Go's rule, chosen for the same reasons. The order of checking is the order of the graph, and a reader always knows which file depends on which.
   - A cycle that a design needs is broken by moving what both files share into a third file.
6. **Names are not renamed.** An imported name that clashes with a name the file declares, or with a name imported from another file, is refused (`E0244`, `clash`), as in managed mode. `import { a as b }` is not part of this decision.
7. **What crosses in this first version:** non-generic functions, structures and enumerations.
   - Generic functions and structures, and module constants, come in a second step, 3b. A generic is made again for each type it is used with, from its source, so the importing file needs that source and not only a signature.
   - Until step 3b, an import of a generic or a constant is refused (`E0244`) with that reason.
8. **The tooling follows the graph.**
   - `perasc build --json` lists the files the module was built from, and the Vite plugin watches each one of them. Changing an imported file rebuilds what imports it.
   - A diagnostic in an imported file names that file and shows its source.
   - A managed module that imports an owned one that imports others needs nothing more: the owned file is built as the root of its own graph.

## Not chosen

- **Allowing cycles.** Rust can because a crate is checked as a whole. ECMAScript can because it resolves cycles at run time, at the cost of reading a value before it is set. Peras checks one file at a time, against signatures, and has no run time in which to resolve anything. Refusing cycles can be relaxed later; allowing them could not be taken back.
- **One WebAssembly module per file, linked when loaded.** Each file would have its own memory and allocator. Every structure or string passed between files would be copied across a boundary, which is what decision 0020 removed for text coming from JavaScript.
- **Renaming with `as`.** It is useful, but not needed to split a program. Without it, a clash is an error, so nothing is lost by adding it later.

## Consequences

- Two `.apeiron` modules that import two owned roots which share a file each get a copy of that file's code, in two WebAssembly modules with two memories. Values pass between them only through JavaScript. Sharing one module between roots is out of scope.
- **Module size.**
  - The files are compiled into one module, and a function no export reaches is still dropped, as today.
  - A program split into files should give the same module as the same program in one file, up to the order of functions.
  - This is checked by the tests below rather than assumed.

## Criteria, registered before any code

The step lands only with all of these, in addition to [decision 0021's criteria](0021-phases-4-and-5.md#criteria-of-each-step):

1. **UI tests for each refusal**, with the importing file's diagnostic:
   - a file not found;
   - a file that does not compile, reported in that file;
   - a name not exported;
   - a clash with a local or imported name;
   - a generic or a constant (until step 3b);
   - a `.apeiron` file imported;
   - a cycle of two files and a cycle of three (`E0250`, with its path).
2. **Equal to one file.**
   - A program split into three files gives the same results as the same program written in one, on both allocators, end to end in Chromium, with nothing left alive.
   - Its module's size is within 1 % of the one-file module's.
3. **Across the boundary between files:**
   - a structure holding strings and arrays passed, moved and returned;
   - an enumeration matched in another file than its own;
   - the same private name in two files;
   - a file imported by two others (a diamond), compiled once.
4. **The differential test** is extended with programs split into two files, run with 2,000 seeds. Breaking the linking on purpose must fail it: one function called in place of another of the same name in another file.
5. **The tooling.**
   - `perasc build --json` lists the files.
   - An end-to-end test of the Vite plugin changes an imported file and sees the rebuilt result.
6. **The guide gains a page on modules**, in English and in French. The status, the roadmap, the reference of `E0244` and the new `E0250` are updated.

## Record

_Each part is recorded here as it lands._
