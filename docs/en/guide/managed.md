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

| Type                  | In JavaScript                                                                                           |
| --------------------- | ------------------------------------------------------------------------------------------------------- |
| `i32`, `u32`, smaller | a `number`, wrapped after each operation: `(a + b) \| 0`, `Math.imul`; checked instead in a debug build |
| `i64`, `u64`          | a `bigint`, wrapped to 64 bits; checked instead in a debug build                                        |
| `f32`                 | a `number`, rounded to `f32` after each operation with `Math.fround`                                    |
| `f64`, `number`       | a `number`                                                                                              |

An integer divided by zero throws the `WebAssembly.RuntimeError` an owned module traps with; an index out of bounds, the same `RangeError`, with the same text. Conversions round toward zero and saturate, as they do in an owned module.

```apeiron build
export fn checksum(xs: Array<i32>): i32 {
  let h = 17;
  for (const x of xs) {
    h = h.wrappingMul(31).wrappingAdd(x);
  }
  return h;
}
```

A hash wraps on purpose, and says so: `wrappingMul` wraps around at 32 bits in every build, as in WebAssembly. Written `h * 31 + x`, it would stop on the first overflow in a debug build, which is what an overflow nobody meant should do.

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

## The browser

A managed module uses the browser directly: `document`, elements, events, a canvas, timers, `fetch` and `storage`, each typed ([browser reference](../reference/browser.md), [decision 0009](../decisions/0009-browser-and-resources.md)). Where the DOM gives `null`, Peras gives `None`, so a missing element is a case the compiler makes you handle:

```apeiron build
export fn greet(name: String): void {
  match (document.getElementById("greeting")) {
    Some(el) => {
      el.textContent = `Hello, ${name}`;
    }
    None => log("no #greeting in the page"),
  }
}
```

An event handler is an arrow function, `button.on("click", (e) => { … })`, or a function of the module by its name, `button.on("click", clicked)`.

## Resources

A listener, a timer and a group are **resources**: each belongs to one variable, and the compiler releases it when that variable goes out of scope, however the block ends. A listener is removed, a timer cleared, a group releases everything added to it. Nothing keeps listening or ticking after its owner is gone, which is how most interfaces leak.

```apeiron build
fn tick(): void {
  log("tick");
}

export fn start(root: Element): Group {
  let group = Group.new();
  const timer = setInterval(1000, () => tick());
  group.add(timer);
  const listener = root.on("click", (e) => e.stopPropagation());
  group.add(listener);
  return group;
}
```

`start` gives its group to its caller, which releases it when the view goes away: `group.release()` in TypeScript, or `using group = start(root)`. A resource can be given away once: returned, added to a group, or released with `release()`. Replacing the resource of a `let` variable releases the one it held, and a `Slot` of a group, `group.slot()`, holds one resource that arrow functions replace with `set`, as a search does to wait for the user to stop typing ([browser reference](../reference/browser.md)). The compiler refuses one kept by nothing, used after it was given away, or kept by an array or an arrow function, where nothing would release it ([E0245](../errors/E0245.md)).

In debug, the default of `perasc build` and Vite's dev server, a resource collected without being released is reported in the console with the file and line that made it.

## Arrow functions

An arrow function sees the variables around it, and changes those declared with `let`. Its body is a value, `(x) => x * 2`, or statements, `(e) => { … }`, which give a value by `return` when its type is written: `(x: i32): i32 => { … }`.

```apeiron build
export fn counter(button: Element, label: Element): Group {
  let group = Group.new();
  let count = 0;
  const show = () => {
    label.textContent = `${count} clicks`;
  };
  group.add(button.on("click", (_e) => {
    count += 1;
    show();
  }));
  show();
  return group;
}
```

Each call of `counter` keeps its own `count`: two buttons, two counters, as a component of a framework. In a managed module an arrow function is also a value: kept in a variable, passed to a function whose parameter has a function type, `(Event) => void`, returned, and called. A function of the module goes there by its name: `button.on("click", clicked)`.

Where nothing gives the types of its parameters, they are written: `const show = (n: i32) => …`. A function type is written for a variable, a parameter or a result, not inside another type, and an exported function does not take or give one ([E0238](../errors/E0238.md)). A resource of the function around it cannot be captured, since the arrow function may run after it was released; one made inside the arrow function is its own, released when it returns ([E0245](../errors/E0245.md)). An owned module keeps arrow functions to the methods of arrays, written in place.

## Calling JavaScript

Anything else, the rest of the DOM or an npm package, is declared in an `extern` block, with Peras types ([decision 0010](../decisions/0010-calling-javascript.md)):

```apeiron build
extern from "./chart.js" {
  type Chart;
  fn makeChart(canvas: CanvasElement): Chart;
  fn plot(self: Chart, values: Array<number>): i32;
  fn onPoint(self: Chart, f: (i32, number) => void): void;
  async fn load(url: String): Promise<Array<number>>;
}

fn picked(i: i32, value: number): void {
  log(`${i}: ${value}`);
}

export async fn show(canvas: CanvasElement): Promise<i32> {
  const chart = makeChart(canvas);
  chart.onPoint((i, value) => picked(i, value));
  return chart.plot(await load("/values.json"));
}
```

- `type Chart;` is a value JavaScript makes, which Peras only passes along.
- A function whose first parameter is `self` of such a type is a method: `chart.plot(values)` calls `plot` on the object JavaScript gave.
- A parameter of a function type, `(i32, number) => void`, takes an arrow function.

Peras checks every value JavaScript gives back against the type declared, in production too: what a function returns, what its promise gives, what JavaScript passes to a callback. A library that gives a string where `i32` is declared throws a `TypeError` that names the function, `peras: Chart.plot(…) returned the string "3" where its declaration says i32`, before any Peras code sees the value. A declaration that is wrong is found where it is wrong. A resource cannot be given to JavaScript, which would never release it ([E0247](../errors/E0247.md)).

The path is a file, `"./chart.js"`, or an npm package, `"date-fns"`, imported as JavaScript imports it: Vite resolves it, and `perasc build` keeps it as written.

## Not yet

- **More of the browser, typed**: forms, `requestAnimationFrame`, WebSockets and workers, until then declared in an `extern` block.
- **Generic `extern` functions**, default exports, and reading `.d.ts` files to propose `extern` blocks.
- **Arrays of resources**, and a request as a resource cancelled with its owner.
- **`async` arrow functions**, an arrow function that calls itself, and functions given to or by JavaScript through an export.
- **Structures and enumerations across an import.**
- **Options of options**, refused with [E0900](../errors/E0900.md): in JavaScript, `Some(None)` and `None` are both `undefined`.
