# Toward a complete language

Peras today covers the core of a language: numbers, structures, functions, control flow, and diagnostics. Building large web applications needs much more. This page lists what is planned, grouped in phases, in the order they are meant to land. Each phase ends with something usable, and every feature arrives the same way as the rest: checked, explained in English and French, tested, and documented here.

The examples on this page show the intended syntax; the compiler refuses them today, and the documentation's tests check that it does, so this page is updated as each feature lands.

## Phase 1: data on the heap

The first types that need memory, on the foundations already built: the release verifier and the debug allocator.

- ~~`Array<T>` compiled: indexing with bounds checks, `push`, `pop`, `for…of`~~: done for every element type, strings and arrays included, with `copy()`.
- ~~`String` compiled, in UTF-8~~: done, with `+`, `+=`, `==`, `length` in bytes and `copy()`.
- ~~`Option<T>` compiled~~: done, with `?` and checked arithmetic.
- ~~Ownership checks: moves, `inout` and `sink`, aliasing refused~~: done ([E0204](../errors/E0204.md), [E0206](../errors/E0206.md), [E0207](../errors/E0207.md), [E0209](../errors/E0209.md)), with `inout` on every type, and the history of a value in errors: where it was given, moved, then used.

## Phase 2: a complete type system

What lets a program say precisely what its data can be.

- ~~**Enumerations with data, and `match`**, checked for exhaustiveness~~: done, see [enumerations and match](../guide/enums.md).
- ~~**`Result<T, E>` and `?`**, for errors that are values~~: done, see [results](../guide/enums.md#results-errors-as-values); parsing functions such as `parse` come with the standard library.
- ~~**Generics**, on functions and structures, with constraints~~: done, on enumerations too, see [generics](../guide/generics.md); with built-in constraints, and interfaces for your own.
- ~~**Methods on structures** (`impl`) and **interfaces** for shared behaviour, resolved at compile time~~: done, see [methods and interfaces](../guide/methods.md).
- ~~Tuples, type aliases, module-level constants compiled~~: done, see [types](types.md#tuples) and [module constants](../guide/basics.md#module-constants).

```peras planned
fn parse_port(text: String): Result<u16, ParseError> {
  const n = text.parse::<u32>()?;
  if (n > 65535) {
    return Err(ParseError.OutOfRange);
  }
  return Ok(n.toU16());
}
```

## Phase 3: a standard library

The everyday tools, with the same rules: explicit types, no hidden allocation, errors as values. In progress: the array methods that take an arrow function, compiled in place as loops, and `min`, `max`, `clamp` are [done](methods.md); the production allocator, TLSF, came first ([decision 0002](../decisions/0002-tlsf-allocator.md)). Text is next: pieces of text as views into the text they come from, then chains of methods fused into one loop, chosen by the compiler ([decision 0004](../decisions/0004-strings-without-allocation.md)).

| Area        | What                                                                                                                                                                                                        |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| text        | ~~`slice`, `split`, `indexOf`, `includes`, `startsWith`, `replace`, `trim`, case conversion, `join`, `toString` on integers and booleans, `parseI32`, `parseI64`, template strings, floats as text~~ (done) |
| arrays      | ~~`map`, `filter`, `reduce`, `find`, `findIndex`, `some`, `every`~~ (done, with arrow functions), ~~`sort`, `slice`, `concat`, `join`, `reverse`, `includes`, `indexOf`~~ (done), iterators and ranges      |
| collections | ~~`Map<K, V>`, `Set<T>`~~ (done, [decision 0005](../decisions/0005-maps-and-sets.md); keys of structures and floats later), `Deque<T>`                                                                      |
| numbers     | ~~`min`, `max`, `clamp`, `pow`, `exp`, `log`, `atan2`, `hypot`~~ (done), constants, seeded random numbers                                                                                                   |
| time        | durations and instants                                                                                                                                                                                      |
| data        | JSON, text encoding, bytes                                                                                                                                                                                  |

```peras build
export fn words(text: String): i32 {
  return text.split(" ").filter((w) => w.length > 0).length;
}
```

## Phase 4: managed mode, for the rest of the application

`.apeiron` modules compiled to JavaScript, for everything that talks to the browser ([spec](../spec.md#managed-mode-apeiron), [decision 0008](../decisions/0008-managed-mode.md)). In progress: the core of the language compiles to JavaScript, with the meaning numbers have in owned modules ([managed modules](../guide/managed.md)).

- ~~`.apeiron` modules compiled to JavaScript, through the CLI and the Vite plugin~~: done, with `number` for `f64`.
- ~~`async` and `await`~~: done.
- ~~Calls from managed to owned modules, with conversions checked at the boundary~~: done for numbers, strings, and arrays, options, results, maps and sets of them; structures and enumerations next.
- ~~The DOM, events, `fetch`, timers, storage, with typed APIs~~: done for a subset, the [browser reference](browser.md) ([decision 0009](../decisions/0009-browser-and-resources.md)); WebSockets, Web Workers, forms and `requestAnimationFrame` next.
- ~~Rules that prevent the classic leaks: a listener or a timer is a resource, released with its owner, and a leak detector reports the ones that are not~~: done.
- ~~Arrow functions with statements that capture what they use, and functions as values~~: done ([guide](../guide/managed.md#arrow-functions)); the case study keeps its state in the table it mounts, shown twice in one page.
- ~~Replacing a resource releases the previous one~~: done, with a `Slot` of a group for an arrow function that replaces one ([browser reference](browser.md)); the case study searches after a delay.
- ~~`sortBy(key)`~~: done, `sortBy` and `sortByDescending` in both modes ([methods](methods.md#sorting-by-a-key)), 3 to 7 times TypeScript's speed by a number.
- ~~`toFixed`~~: done, in both modes, JavaScript's text by construction ([methods](methods.md#as-text)); the cost on the machine is measured with every step from now on ([decision 0012](../decisions/0012-cost-on-the-machine.md)).
- Next, in the order [decision 0011](../decisions/0011-what-to-build-next.md) measured: integer overflow stopped in debug, once its cost is measured; structures across `.apeiron` and `.peras`; elements owned by a group.

```apeiron planned
import { inside } from "./mandel.peras";

export async fn render(canvas: HTMLCanvasElement): Promise<void> {
  const response = await fetch("/settings.json");
  const settings = Settings.fromJson(await response.text())?;
  draw(canvas, settings, inside);
}
```

## Phase 5: modules and packages

- `import` and `export` between Peras modules.
- ~~Typed use of JavaScript libraries~~: done with `extern` blocks, each value from JavaScript checked against the types declared ([decision 0010](../decisions/0010-calling-javascript.md)); `extern` blocks proposed from `.d.ts` declarations next.
- Packages: a manifest, versions, a registry.

## Phase 6: graphics and performance

What the spec calls rasterization ([spec](../spec.md#rasterization)).

- `Framebuffer<Rgba8>` and pixel types, shared with the canvas without copying.
- SIMD types and operations.
- The `@frame` rule: a function that renders a frame may not allocate, and the compiler proves it.
- `Arena` and `Pool<T>` allocators.
- WebGPU bindings.

## Phase 7: tooling

- An editor extension and a language server: errors as you type, completion, hover, go to definition, renaming.
- A formatter, so that every Peras file looks the same.
- A test runner: tests written in Peras, run by Vitest.
- `create-peras`, to start a project in one command.
- A playground in this documentation: the compiler itself compiled to WebAssembly, running in the page.
- Two tools apart from the open core, proprietary ([decision 0006](../decisions/0006-licensing.md)): the VS Code extension and a memory leak analyzer, built on what Peras publishes. The leak checks of the debug allocator stay in the core.

## Before using Peras in production

An outside review of the project, in November 2026, advised against using it in production yet. Its facts that hold, checked in this repository, are kept here; each is work for the phases above, not a verdict on where the language is going. The published research on adopting a new language, which bears on several of them, is in [the research behind Peras](../research.md#adoption-and-its-risks).

- **Version 0.0.1, no stability promise**: until 1.0, any version may break the language. A written policy, saying what may change and how a change is announced, comes before any outside user.
- **Half of the design was missing**: `.apeiron` modules now compile to JavaScript and call owned modules ([managed modules](../guide/managed.md)), but the browser is not typed yet (phase 4), and `std/` holds only its README.
- **One maintainer**: the project depends on one person, helped by an AI assistant; it has no community yet.
- **No libraries of its own**: the plan is to use those of npm, not to rewrite them; a managed module calls them through `extern` blocks, each declared by hand ([decision 0010](../decisions/0010-calling-javascript.md)).
- **Workers, but no shared memory; no GPU**: a module runs in Web Workers as it is, one instance each, measured 2.6 to 2.9 times faster on 4 cores where the work outweighs copying the data and slower where it does not ([Web Workers](../../../bench/README.md#web-workers)); memory shared between workers is not built yet (decision [0003](../decisions/0003-runtime-proved-at-compile-time.md) plans moves between threads), nor WebGPU (phase 6).
- **Ownership is learned**: diagnostics explain every refusal, with the value's history and the fix, but there is no community knowledge to lean on yet.
- **Compared with Rust and AssemblyScript as usually written, not yet with their experts**: Peras runs at the speed of idiomatic Rust, ahead where its compiler does more (SIMD loops with branches, radix sort, text crossing), and 2.4 to 7.3 times faster than AssemblyScript where it allocates or loops ([against Rust and AssemblyScript](../../../bench/README.md#against-rust-and-assemblyscript)).
- **A licence was missing**: settled by [decision 0006](../decisions/0006-licensing.md).

Today, Peras fits where its risk stays small: a few functions of computation, compiled to WebAssembly and called from a TypeScript application through the Vite plugin.

## Why this order

1. **Data first**: almost every program needs arrays and text, and the memory foundations for them are ready.
2. **Types next**: enumerations, `match`, `Result` and generics change how every later API is written; the standard library is designed on top of them, not before.
3. **The standard library** then gives each type the methods that make it useful.
4. **Managed mode** opens the browser: the DOM and `async` make Peras an application language, not only a computation language.
5. **Modules, graphics and tooling** follow, the editor support starting as soon as the type system is stable.

What already works is on the [status page](status.md).
