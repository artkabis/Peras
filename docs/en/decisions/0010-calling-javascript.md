# 0010: Calling JavaScript, with declared types checked at the boundary

_French translation: [../../fr/decisions/0010-calling-javascript.md](../../fr/decisions/0010-calling-javascript.md)._

## Context

After [decision 0009](0009-browser-and-resources.md), a managed module reaches a subset of the browser typed by hand, and nothing else: no other API, no npm package. An assessment of the project found this the first obstacle to using it in a real application, and the research on adoption agrees: libraries and existing code weigh most in the choice of a language ([the research behind Peras](../research.md#adoption-and-its-risks)). Typing the browser member by member cannot keep up.

Peras needs a way out to JavaScript that keeps its guarantees: what a Peras function receives must be what its type says, even when it comes from a library that says otherwise.

## What others have done

- **TypeScript** trusts its declarations: a `.d.ts` that is wrong, or a library that changes, gives values of another type at run time, and nothing notices.
- **Elm** lets JavaScript in only through ports and flags, whose values are decoded and checked; a wrong value is an error at the boundary, never inside the program.
- **ReScript** binds JavaScript with external declarations, trusted as TypeScript's are.
- **io-ts and Zod** check values against a schema at run time, at the cost of writing the schema.

## Decision

1. **`extern` blocks.** A managed module declares the JavaScript it calls with Peras types, from a file or an npm package:

   ```apeiron
   extern from "./chart.js" {
     type Chart;
     fn makeChart(canvas: CanvasElement): Chart;
     fn plot(self: Chart, values: Array<number>): i32;
     fn onPoint(self: Chart, f: (i32, number) => void): void;
     async fn load(url: String): Promise<Array<number>>;
   }
   ```

   `type Chart;` is a value JavaScript makes and Peras only passes. A function whose first parameter is `self` of such a type is a method of its values. An owned module cannot declare one ([E0242](../errors/E0242.md)).

2. **Every value from JavaScript is checked, always.** What an `extern` function returns, what its promise gives, and what JavaScript passes to a callback is checked against the type declared, in production too: numbers and their ranges, strings, booleans, arrays element by element, options (`null` and `undefined` are `None`), maps, sets, the fields of a structure, the variants of an enumeration, `instanceof` for the browser's types. The value is rebuilt from what the type declares, so that JavaScript keeps no hold on it. A value that does not match throws a `TypeError` that names the function and the type declared, before Peras code ever sees it.
3. **Callbacks are arrow functions**, passed where the `extern` function is called, with the parameters' types the declaration gives; JavaScript calls them through a wrapper that checks its arguments.
4. **Resources do not cross.** A listener, a timer or a group belongs to one variable of Peras, which releases it; JavaScript would keep it without releasing it ([E0247](../errors/E0247.md)).

## Not chosen

- **Trusting the declarations**, as TypeScript does: no cost, and a wrong declaration or a library that changes breaks Peras's guarantees silently.
- **Checking in debug only**: a library that misbehaves in production would go unnoticed, where the bug costs most.
- **Reading `.d.ts` files**: they would give the declarations, not the checks, and their `any`, unions and overloads do not translate. They may later propose `extern` blocks to check and adjust.

## Status

**Built.**

- **Syntax**: `extern from "…" { type …; [async] fn …; }`, and function types `(A, B) => R` in its parameters.
- **Checks**: opaque types, functions, methods by `self`, callbacks written as arrow functions, and the four refusals of E0247.
- **Generator**: each function is imported under a name of its own and called through a wrapper that checks what it returns, and each callback through one that checks its arguments (`$callback`). Each type declared is compiled once, when the module loads, into a function that checks a value of that type (`$shape`, `$valid`).
- **Tests**: in Node, a library's functions, methods, promise, option, structure and callbacks, and five that lie, each refused with its message; in Chromium, from Vite's dev server and its production build, an npm package imported by its name draws on a canvas, calls back, gives back a canvas checked with `instanceof`, and one of its methods that lies throws.

Measured cost, in Node, an `extern` function returning an array of a million numbers, medians of 15 calls: 3.8 to 6.6 ms to check and copy it, against 3 ms for JavaScript's `slice()` and 15 ms for a `map` over it. A first version read the description of the type for each element, and took 77 ms; compiling each type into a function once, when the module loads, made it 12 to 20 times faster. A `void` result is not checked.

Not done: generic `extern` functions, default exports, values (not functions) exported by a module, and reading `.d.ts` files.

## Consequences

- Any JavaScript, the rest of the DOM and npm included, can be called from a managed module, its values kept to the types declared.
- A declaration that is wrong is found the first time the program meets it, with the function's name, instead of somewhere later.
