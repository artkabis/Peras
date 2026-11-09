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

The everyday tools, with the same rules: explicit types, no hidden allocation, errors as values. In progress: the array methods that take an arrow function, compiled in place as loops, and `min`, `max`, `clamp` are [done](methods.md); the production allocator, TLSF, came first ([decision 0002](../decisions/0002-tlsf-allocator.md)).

| Area        | What                                                                                                                                                                                                   |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| text        | ~~`slice`, `split`, `indexOf`, `includes`, `startsWith`, `replace`, `trim`, case conversion, `join`~~ (done), formatting and template strings                                                          |
| arrays      | ~~`map`, `filter`, `reduce`, `find`, `findIndex`, `some`, `every`~~ (done, with arrow functions), ~~`sort`, `slice`, `concat`, `join`, `reverse`, `includes`, `indexOf`~~ (done), iterators and ranges |
| collections | `Map<K, V>`, `Set<T>`, `Deque<T>`                                                                                                                                                                      |
| numbers     | ~~`min`, `max`, `clamp`, `pow`, `exp`, `log`, `atan2`, `hypot`~~ (done), constants, seeded random numbers                                                                                              |
| time        | durations and instants                                                                                                                                                                                 |
| data        | JSON, text encoding, bytes                                                                                                                                                                             |

```peras build
export fn words(text: String): i32 {
  return text.split(" ").filter((w) => w.length > 0).length;
}
```

## Phase 4: managed mode, for the rest of the application

`.apeiron` modules compiled to JavaScript, for everything that talks to the browser ([spec](../spec.md#managed-mode-apeiron)).

- The DOM, events, `fetch`, timers, storage, WebSockets, Web Workers, with typed APIs.
- `async` and `await`.
- Closures, with rules that prevent the classic leaks: a listener or a timer is a resource, released with its owner, and a leak detector reports the ones that are not.
- Calls between managed and owned modules, with conversions checked at the boundary.

```peras planned
import { inside } from "./mandel.peras";

export async fn render(canvas: HTMLCanvasElement): Promise<void> {
  const response = await fetch("/settings.json");
  const settings = Settings.fromJson(await response.text())?;
  draw(canvas, settings, inside);
}
```

## Phase 5: modules and packages

- `import` and `export` between Peras modules.
- Typed use of JavaScript libraries, from their `.d.ts` declarations.
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

## Why this order

1. **Data first**: almost every program needs arrays and text, and the memory foundations for them are ready.
2. **Types next**: enumerations, `match`, `Result` and generics change how every later API is written; the standard library is designed on top of them, not before.
3. **The standard library** then gives each type the methods that make it useful.
4. **Managed mode** opens the browser: the DOM and `async` make Peras an application language, not only a computation language.
5. **Modules, graphics and tooling** follow, the editor support starting as soon as the type system is stable.

What already works is on the [status page](status.md).
