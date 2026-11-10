# Peras and JavaScript

A Peras module is used from JavaScript or TypeScript like any ES module.

## What `perasc build` writes

For `geometry.peras`:

| File            | Contents                                                             |
| --------------- | -------------------------------------------------------------------- |
| `geometry.wasm` | the WebAssembly module                                               |
| `geometry.js`   | an ES module that loads it and exports the functions marked `export` |
| `geometry.d.ts` | TypeScript declarations                                              |

## Types across the boundary

| Peras                 | JavaScript                                                                                                 |
| --------------------- | ---------------------------------------------------------------------------------------------------------- |
| `i32`, `f32`, `f64`   | `number`                                                                                                   |
| `i64`                 | `bigint`                                                                                                   |
| `bool`                | `boolean`                                                                                                  |
| a structure           | a plain object, declared as an `interface`                                                                 |
| `Option<T>`           | the value, or `undefined`                                                                                  |
| `Array<T>`            | an array, of objects for structures, copied in and out                                                     |
| `String`              | a string, copied as UTF-8                                                                                  |
| `Map<K, V>`, `Set<K>` | a `Map` or a `Set`, copied in insertion order, in and out; an `inout` one gets the function's entries back |

```peras build
export struct Rect {
  width: f64;
  height: f64;
}

export fn area(r: Rect): f64 {
  return r.width * r.height;
}

export fn big(r: Rect): bool {
  return area(r) > 100.0;
}
```

```ts
import { area, big } from "./shapes.peras";

area({ width: 3, height: 4 }); // 12
big({ width: 30, height: 4 }); // true
```

The loader converts at the boundary: an object passed in is copied field by field, so Peras never keeps a reference to it, and changes made by Peras never reach it. An array is copied the same way, element by element; only an `inout` parameter is copied back into the array passed ([collections](collections.md#in-javascript)).

## Arrays without a copy

An array of numbers passed to an exported function is copied into the module's memory, and back for an `inout` one: O(n) each call. For large arrays, a `PerasArray` stays in the module's memory and is passed with no copy. <span class="status compiled">compiled</span>

```js
import { scale, sum, __perasHeap } from "./points.peras";

const xs = __perasHeap.array("f32", 1_000_000); // zeros; or from an array of numbers
xs.view().set(samples); // a Float32Array over the module's memory
scale(xs, 2); // inout: changed in place, nothing copied back
const total = sum(xs); // lent: nothing copied in
xs.release(); // gives its memory back
```

- `__perasHeap.array(kind, init)` makes one: `kind` is `"i32"`, `"i64"`, `"f32"`, `"f64"`, `"bool"` or `"u8"`, and `init` a length (zeros) or an array of values.
- Every exported function that takes an array of numbers accepts one: lent to a plain parameter, changed in place by an `inout` one, given to a `sink` one, after which it can no longer be used.
- `view()` is a typed array over its elements, with no copy. It is valid until the next call into the module, which may grow its memory or move the elements: take a new view after each call.
- `toArray()` copies it into a JavaScript array; `length` and `kind` describe it.
- `release()` gives its memory back at once. One never released is given back when the garbage collector collects it.

On a million `f32`, `sum` goes from 5.2 to 1.5 ms, and `scale`, which changes its array, from 11.5 to 0.8 ms: the copies were most of the time.

## Vector loops

A loop whose turns are independent runs four `i32` or `f32`, or two `f64`, at once, with WebAssembly's 128-bit vectors; the loop as written finishes the last turns. The result is exactly the one the loop as written gives: Peras vectorizes only what every lane computes the same way. An `if` that compares numbers is vectorized too: both branches run in every lane, and each lane keeps the result of the branch it would have taken. In `bench/browser`, a grayscale filter on 4 million pixels goes from about 10 to 5 ms, twice as fast as the same JavaScript.

```peras build
export fn gain(samples: inout Array<f32>, g: f32): void {
  for (let i = 0; i < samples.length; i++) {
    samples[i] = samples[i] * g;
  }
}
```

The runtime uses them too, to look for a byte sixteen at a time: `split` on a million characters takes 35 % less time.

WebAssembly SIMD is on by default. A module that uses it, in its loops or its runtime, needs Chrome and Edge 91, Firefox 89, Safari 16.4, Node 16.4 or later; elsewhere, its loader throws an error that says so before loading anything. `perasc build --simd off`, or `simd: false` for Vite, builds without it, for engines that have none, such as Safari before 16.4 on iPhones that cannot update past iOS 15.

## Threads

`perasc build --threads on`, or `threads: true` for Vite, runs the turns of each [`parallel for`](control-flow.md#parallel-for) loop on every core ([decision 0007](../decisions/0007-threads-over-shared-memory.md)). The module, its runtime and a pool of workers share one memory: no data is copied. The page calls the loop, gives a part of its turns to each worker, runs its own part, and waits for them.

Threads need a page that can share memory, served with two headers:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Vite's dev server sends them when `threads` is on; in production, the server must. Elsewhere, in Node or on a page without them, the module loads all the same and runs every loop on one thread, with the same results. `__perasHeap.threads()` says how many threads run them.

In `bench/browser`, on 4 cores, 200 steps of a million particles run 2.7 to 2.8 times faster than on one thread, 10 steps 2 to 2.2 times, a grayscale filter over 16 MB 1.7 to 1.9 times, its checksum included, which a second loop computes on one thread.

## With Vite

```ts
// vite.config.ts
import { defineConfig } from "vite";
import peras from "vite-plugin-peras";

export default defineConfig({
  plugins: [peras({ perasc: "path/to/perasc", lang: "en" })],
});
```

| Option      | Default                                             | Meaning                                                                                            |
| ----------- | --------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `perasc`    | `PERASC`, then `perasc` on the `PATH`               | the compiler to run                                                                                |
| `lang`      | the system's                                        | `"en"` or `"fr"` for diagnostics                                                                   |
| `dts`       | `true`                                              | write `name.d.peras.ts` next to each `name.peras`, `name.d.apeiron.ts` next to each `name.apeiron` |
| `allocator` | `debug` for the dev server, `tlsf` for `vite build` | the allocator of modules that use the heap                                                         |
| `simd`      | `true`                                              | WebAssembly SIMD in loops and the runtime; `false` for engines without it                          |
| `threads`   | `false`                                             | `parallel for` loops on every core; the dev server isolates the page                               |

In development the WebAssembly is inlined in the module; `vite build` emits it as a separate file, compiled while it downloads. An error in a `.peras` file shows in Vite's overlay with its excerpt, cause and fix, and fails `vite build`. The plugin compiles `.apeiron` files too, to JavaScript ([managed modules](managed.md)).

## Memory errors at run time

In a module that uses the heap, the debug allocator throws a `PerasMemoryError` on a memory bug, with the sites involved in its message and as `allocSite` and `releaseSite`. Such a module also exports `__perasHeap`:

```ts
import { __perasHeap } from "./parser.peras";

__perasHeap.live(); // blocks still alive
__perasHeap.checkLeaks(); // throws if one is
```

An index out of bounds throws a `RangeError`, in the language the module was built in:

```text
RangeError: index 3 is out of bounds: the length is 3
```

This is the end of the guide. The [reference](../reference/status.md) lists everything the language has today.
