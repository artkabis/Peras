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

| Peras               | JavaScript                                 |
| ------------------- | ------------------------------------------ |
| `i32`, `f32`, `f64` | `number`                                   |
| `i64`               | `bigint`                                   |
| `bool`              | `boolean`                                  |
| a structure         | a plain object, declared as an `interface` |
| `Option<T>`         | the value, or `undefined`                  |
| `Array<T>`          | an array, copied in and out                |

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

## With Vite

```ts
// vite.config.ts
import { defineConfig } from "vite";
import peras from "vite-plugin-peras";

export default defineConfig({
  plugins: [peras({ perasc: "path/to/perasc", lang: "en" })],
});
```

| Option   | Default                               | Meaning                                           |
| -------- | ------------------------------------- | ------------------------------------------------- |
| `perasc` | `PERASC`, then `perasc` on the `PATH` | the compiler to run                               |
| `lang`   | the system's                          | `"en"` or `"fr"` for diagnostics                  |
| `dts`    | `true`                                | write `name.d.peras.ts` next to each `name.peras` |

In development the WebAssembly is inlined in the module; `vite build` emits it as a separate file, compiled while it downloads. An error in a `.peras` file shows in Vite's overlay with its excerpt, cause and fix, and fails `vite build`.

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
