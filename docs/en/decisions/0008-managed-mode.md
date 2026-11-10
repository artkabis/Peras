# 0008: Managed mode, compiled to JavaScript

_French translation: [../../fr/decisions/0008-managed-mode.md](../../fr/decisions/0008-managed-mode.md)._

## Context

The [spec](../spec.md#managed-mode-apeiron) gives Peras two modes, set by the file's extension: owned modules (`.peras`), compiled to WebAssembly, and managed modules (`.apeiron`), compiled to JavaScript for what talks to the browser. Owned modules are built. `perasc-codegen-js` is an empty crate, and the Vite plugin refuses `.apeiron` files. An application still writes its interface in TypeScript, and calls its owned modules through a loader that converts each value at the boundary.

This decision fixes what a managed module is, and what its first step builds.

## What others have done

- **ReScript and Elm** compile a typed language to readable JavaScript, with their own data model, and call JavaScript through declared bindings.
- **Kotlin/JS and Scala.js** compile a JVM language to JavaScript: exact `Long` costs an object per value, so `Int` arithmetic is wrapped, and `Long` is emulated.
- **AssemblyScript** is TypeScript-shaped and compiles to WebAssembly only; an application mixes it with TypeScript by hand.
- **TypeScript** erases its types: a `number` annotated `i32` does not exist; nothing checks what a module receives from JavaScript.

## Decision

1. **The garbage collector owns ordinary values.** A managed module has no moves and no borrows: `let ys = xs` shares the array, as in JavaScript. `sink`, `parallel for`, and `inout` on a value JavaScript copies are refused there ([E0243](../errors/E0243.md)). Ownership comes back, later, for **resources** only: listeners, timers, sockets, as the spec plans.
2. **Numbers keep their meaning.** `number` is a managed module's name for `f64`. `i32`, `i64` and `f64` keep the semantics they have in an owned module, so that a function moved from one mode to the other computes the same: an `i32` wraps, `(a + b) | 0` and `Math.imul`; an `i64` is a `BigInt`, wrapped with `BigInt.asIntN(64, …)`; a division by zero throws as it traps.
3. **Strings are JavaScript's.** `length` and `slice` count UTF-16 code units in a managed module, bytes of UTF-8 in an owned one; the text is the same, its indices are not. A managed module does not convert its strings: the DOM and `fetch` speak UTF-16.
4. **Data keeps the shapes of the boundary.** A structure is an object with its fields, an enumeration `{ kind, values }`, `Option<T>` a value or `undefined`, a tuple an array, `Map` and `Set` JavaScript's: the shapes the owned loader already gives. A value crosses from a managed module to an owned one without a format of its own.
5. **`async` and `await`, in managed modules only.** An `async fn` returns `Promise<T>`, and its body returns `T`; `await` is allowed only inside one ([E0242](../errors/E0242.md)). An owned module never waits.
6. **A managed module imports owned functions.** `import { mean } from "./stats.peras"` checks the call against the function's signature, read from the owned module itself, and goes through its loader. Numbers, booleans, strings, and arrays, options, results, maps and sets of them cross; a structure or an enumeration is the other module's type and does not cross yet ([E0244](../errors/E0244.md)). An owned module never calls a managed one.

## Step 1, and what comes after

Step 1 builds the core: functions, structures, enumerations and `match`, arrays, strings, maps and sets, `async` and `await`, compiled to JavaScript, and the imports of owned modules, through the CLI and the Vite plugin.

Step 2 types the browser: the DOM, events, `fetch` and timers, then resources and their release, with the leak detector the spec describes.

## Not chosen

- **WasmGC.** It shares the JavaScript garbage collector without freeing earlier, and makes interop and debugging harder ([spec](../spec.md#managed-mode-apeiron)).
- **`number` as the only number.** JavaScript's own choice would make a function compute differently in each mode; `number` is offered, as `f64`.
- **Strings in UTF-8 everywhere.** Each DOM call would convert.

## Status

**Step 1 built.** `perasc build app.apeiron` writes `app.js` and `app.d.ts`, and builds the owned modules it imports; the Vite plugin compiles `.apeiron` files, and writes `app.d.apeiron.ts`.

- **The checks.** A managed module skips the ownership pass. `number` and `Promise<T>` resolve only there; an `async fn` declares `Promise<T>` and its body returns `T`. `import` reads the owned module beside it, checks it, and declares each exported function with its signature; the path must name a `.peras` file in the same folder or below it.
- **The generator** (`perasc-codegen-js`) writes JavaScript from the syntax tree and the checker's types, with the name each variable resolves to: every local gets a name unique in its function, and a word JavaScript reserves gets a `$`. Generic functions are compiled once per instance, as for WebAssembly. A `match` or a `?` that is a value writes its statements before the expression, the operands before it kept in temporaries so that the order of evaluation does not change, and `&&` runs them only on its path. Runtime helpers (`$at`, `$div`…) are written into a module only when it uses them.
- **The same results.** One source compiled both ways, `kernels.peras` and `kernels.apeiron`, gives the same results on 21 calls in Node: wrapping `i32` arithmetic, `i64`, `f32`, saturating conversions, maps, sorting, `match`, `?`, templates, and the same errors, a `RangeError` with the same text for an index out of bounds, a `WebAssembly.RuntimeError` for a division by zero. In Chromium, a managed module calling an owned one runs from Vite's dev server and from its production build.
- Every source of the UI tests, the end-to-end fixtures and the documentation that checks as a managed module, 104 of them, compiles to JavaScript that Node parses; the fuzz target compiles any text as a managed module too.

- **The speed of hand-written JavaScript.** `node scripts/bench-managed.mjs` runs four kernels compiled both ways against the same work written by hand, in Node: the managed module runs at 0.93 to 1.31 times its speed, the owned one 1.02 to 1.91 times, except where it copies JavaScript arrays in and out ([results](../../../bench/README.md#managed-modules-benchmanaged)). Bounds checks are proven away in a counted loop over an array whose body calls nothing, as in WebAssembly: without, the sum ran at 0.83 – 0.86.
- **`inout` in a managed module.** A parameter is read-only in both modes; an `inout` one, of a type JavaScript shares (an array, a map, a set, a structure), is changed in place for its caller, and cannot be replaced by another value. One JavaScript copies is refused ([E0243](../errors/E0243.md)).

Not done: the typed browser and resources (step 2), functions as values, structures and enumerations across an import, options of options, refused with `E0900` since JavaScript cannot tell `Some(None)` from `None`.

## Consequences

- A function's results are the same in both modes, except for the indices of strings, which count UTF-16 units in a managed module.
- Exact `i64` arithmetic costs a `BigInt` in a managed module; `number` is the fast choice there.
