# Managed modules

A Peras file is one of two kinds, set by its extension ([decision 0008](../decisions/0008-managed-mode.md)):

- an **owned** module, `.peras`, compiles to WebAssembly: memory it manages itself, checked at compile time, for computation;
- a **managed** module, `.apeiron`, compiles to JavaScript: values the garbage collector manages, for the rest of the application. It waits with `async` and `await`, and calls owned modules by importing them.

Both are written in the same language. A managed module is compiled to a plain ES module, which runs wherever JavaScript does. <span class="status compiled">compiled</span>

```apeiron build
import { mean } from "./stats.peras";

async fn load(n: i32): Promise<Array<number>> {
  let xs: Array<number> = [];
  for (let i = 0; i < n; i++) {
    xs.push(i.toF64() / 10.0);
  }
  return xs;
}

export async fn report(n: i32): Promise<String> {
  const xs = await load(n);
  return `mean ${mean(xs)} of ${xs.length} values`;
}
```

`mean` runs in WebAssembly; `report` in JavaScript.

## Values belong to the garbage collector

In a managed module, nothing moves and nothing is lent: assigning or passing an array, a map, a set or a structure shares it, as in JavaScript. A parameter is read-only, as in an owned module; an `inout` one is changed in place, and its caller sees the change:

```apeiron build
export fn record(log: inout Array<String>, line: String): void {
  log.push(line);
}

export fn twice(): i32 {
  let log: Array<String> = [];
  record(&log, "a");
  record(&log, "b");
  return log.length;
}
```

A number, a boolean, a string or an option is copied when it is passed: an `inout` one could not reach its caller, and is refused, as is `sink`, which gives a value away, and `parallel for`, which shares memory between threads ([E0243](../errors/E0243.md)):

```apeiron error E0243
export fn bump(n: inout i32): void {
  n += 1;
}
```

Ownership will come back to managed modules for **resources** only: listeners, timers, sockets, released with their owner.

## Numbers keep their meaning

`number` is the managed name of `f64`. The other number types compute exactly as in an owned module, so a function moved from one mode to the other gives the same results:

| Type                  | In JavaScript                                                         |
| --------------------- | --------------------------------------------------------------------- |
| `i32`, `u32`, smaller | a `number`, wrapped after each operation: `(a + b) \| 0`, `Math.imul` |
| `i64`, `u64`          | a `bigint`, wrapped to 64 bits                                        |
| `f32`                 | a `number`, rounded to `f32` after each operation with `Math.fround`  |
| `f64`, `number`       | a `number`                                                            |

An integer divided by zero throws the `WebAssembly.RuntimeError` an owned module traps with; an index out of bounds, the same `RangeError`, with the same text. Conversions round toward zero and saturate, as they do in an owned module.

```apeiron build
export fn checksum(xs: Array<i32>): i32 {
  let h = 17;
  for (const x of xs) {
    h = h * 31 + x;
  }
  return h;
}
```

`h * 31` wraps around at 32 bits, as in WebAssembly.

## Strings are JavaScript's

A managed module keeps its strings as JavaScript has them: the DOM and `fetch` speak UTF-16, and nothing is converted. `length`, `slice` and `indexOf` therefore count UTF-16 units there, where an owned module counts bytes of UTF-8: `"é".length` is 1 in a managed module, 2 in an owned one. The text is the same; only its indices differ. The other methods give the same results in both modes: `toUpperCase` changes ASCII letters only, `trim` removes spaces, tabs and line breaks, `split("")` gives the characters.

## Waiting

An `async` function returns a `Promise<T>`; its body returns a `T`. `await` waits for a promise, inside an `async` function only ([E0242](../errors/E0242.md)):

```apeiron error E0242
async fn load(): Promise<number> {
  return 1.0;
}

export fn now(): number {
  return await load();
}
```

An owned module never waits: it computes and returns, and `async`, `await` and `Promise` are refused there.

## Importing owned modules

A managed module imports exported functions from owned modules, by their relative path, in the same folder or below it:

```apeiron build
import { mean } from "./stats.peras";

export fn centered(xs: Array<number>): Array<number> {
  const m = mean(xs);
  return xs.map((x) => x - m);
}
```

Each call is checked against the function's signature, read from the owned module, and converted by its loader as for any JavaScript caller ([types across the boundary](javascript.md#types-across-the-boundary)). Numbers, booleans, strings, and arrays, options, results, maps and sets of them cross. A structure or an enumeration is the other module's own type and does not cross yet ([E0244](../errors/E0244.md)): give the function numbers or arrays, and build the structure on each side.

An owned module cannot import a managed one: computation does not call the interface.

## Building

`perasc build app.apeiron` writes `app.js` and `app.d.ts`, and builds each owned module it imports at the same relative path: `./lib/stats.peras` becomes `lib/stats.wasm`, `lib/stats.js` and `lib/stats.d.ts`, which `app.js` imports. `--imports source` keeps the imports of `.peras` files as they are, for a bundler that compiles them itself.

With Vite, the plugin compiles both kinds: import `./app.apeiron` from TypeScript as any module. It writes `app.d.apeiron.ts` next to it, for TypeScript.

```ts
import { report } from "./app.apeiron";

document.body.textContent = await report(10);
```

## Not yet

- **The browser, typed**: the DOM, events, `fetch`, timers and storage, with their types, come in the next step. Until then, a managed module receives and returns values, and TypeScript code calls the browser.
- **Resources**: listeners and timers owned and released, with a detector of leaks in debug.
- **Functions as values**, outside the arrow functions given to array methods.
- **Structures and enumerations across an import.**
- **Options of options**, refused with [E0900](../errors/E0900.md): in JavaScript, `Some(None)` and `None` are both `undefined`.
