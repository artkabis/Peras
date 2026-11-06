# Introduction

Peras is a language for building web applications whose critical parts must be fast, predictable and correct: rendering, simulation, audio, data processing, games. It reads like TypeScript, and checks like Rust.

## Two modes, one language

A Peras module is in one of two modes, set by its file extension:

| Mode        | Extension  | Compiled to | Memory                                                                                            |
| ----------- | ---------- | ----------- | ------------------------------------------------------------------------------------------------- |
| **Owned**   | `.peras`   | WebAssembly | exact: every value has one owner, released at the end of its scope, no garbage collector          |
| **Managed** | `.apeiron` | JavaScript  | the browser's garbage collector, with ownership applied to resources such as listeners and timers |

Owned mode is for the code where every millisecond and every byte count. Managed mode, planned, is for the rest of the application: the DOM, events, network calls. Both share the syntax, the types and the diagnostics.

> Today, owned mode compiles functions on numbers, booleans and structures. Managed mode is designed but not built yet. The [status page](../reference/status.md) lists every feature.

## What makes Peras different

**Nothing implicit.** Peras never converts a value's type on its own: `1 + 2.5` is an error, and you write `1.0 + 2.5` or `n.toF64() + 2.5`. There is no `undefined`, no `null`, no truthiness: a condition is a `bool`.

**Every mistake stops the build.** There are no warnings. An unused variable, a shadowed name, a missing return: each one is an error with a stable code, an excerpt of your source, the cause, and the fix as a diff.

**Errors that teach.** Every diagnostic explains why the rule exists, in English or French. `perasc explain E0006` shows the full page.

**Fast where it matters.** Owned modules compile to WebAssembly with Peras's own optimizer, measured against Rust on the same programs. The [benchmark](https://github.com/artkabis/Peras/blob/main/bench/backends/RESULTS.md) is in the repository.

**No toolchain to install.** `perasc` is one program. It writes the `.wasm` module, a JavaScript loader and TypeScript declarations; a Vite plugin imports `.peras` files like any module.

## A first taste

```peras build
// Is a point inside the Mandelbrot set?
export fn inside(cx: f64, cy: f64): bool {
  let x = 0.0;
  let y = 0.0;
  for (let step = 0; step < 64; step++) {
    if (x * x + y * y > 4.0) {
      return false;
    }
    const next = x * x - y * y + cx;
    y = 2.0 * x * y + cy;
    x = next;
  }
  return true;
}
```

Next: [getting started](getting-started.md).
