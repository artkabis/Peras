# 0009: The browser, typed, and resources released with their owner

_French translation: [../../fr/decisions/0009-browser-and-resources.md](../../fr/decisions/0009-browser-and-resources.md)._

## Context

Step 1 of managed mode ([decision 0008](0008-managed-mode.md)) compiles `.apeiron` modules to JavaScript, but they could not reach the browser: a managed module received values and returned them, and TypeScript called the DOM. The [spec](../spec.md#managed-mode-apeiron) plans two things for step 2: typed browser APIs, and ownership for **resources** only, the listeners, timers and requests whose leaks are the usual ones in a web application, a reference that stays attached rather than a missing free.

## What others have done

- **TypeScript** types the whole DOM from `lib.dom.d.ts`: complete, with `any`, unions and overloads that the checker accepts loosely.
- **Elm and ReScript** bind a chosen part of the browser by hand, each binding written and checked once.
- **Explicit resource management** (TC39, in V8 since 2025) adds `using` and `Symbol.dispose` to JavaScript: a resource bound with `using` is disposed when its block ends. Nothing checks that a resource is bound at all.
- **`AbortController`**: one signal given to `addEventListener` and `fetch` removes and cancels everything it was given to.
- **`FinalizationRegistry`** tells a program that an object was collected: tools use it to find leaks in development.

## Decision

1. **A subset of the browser, typed by hand.** `Document`, `Element`, `InputElement`, `CanvasElement`, `Context2d`, `Event`, `Response` and `Storage`, the globals `document`, `storage`, `fetch`, `sleep`, `now`, `log`, `setTimeout` and `setInterval`, each member listed once in `perasc-check` ([browser reference](../reference/browser.md)). Where the DOM gives `null`, Peras gives `None`; `asInput()` and `asCanvas()` are checked casts. Properties the browser sets are read only ([E0246](../errors/E0246.md)). An owned module cannot use the browser ([E0242](../errors/E0242.md)). More APIs come when a program needs them, each added the same way.
2. **Resources have one owner.** A `Listener`, a `Timer` and a `Group` belong to one variable, and are released when it goes out of scope, by any way out. A resource can be given away once: returned, added to a `Group`, or released with `release()`. The checker refuses a resource kept by nothing, used after it was given away, given away in a loop, kept by a structure, an array, an option, a map or an arrow function, given away by a function that only borrowed it, or replaced ([E0245](../errors/E0245.md)).
3. **Released as JavaScript releases them.** A listener is added with the signal of its own `AbortController`, and released by aborting it; a timer is cleared. The rest of the owner's block runs in a `try` whose `finally` releases the resource, unless a flag says it was given away. A resource is an object with `release()` and `[Symbol.dispose]()`, so that TypeScript releases one a module returns, or holds it with `using`.
4. **Leaks reported in debug.** With the debug allocator, the default of `perasc build` and Vite's dev server, each resource is registered with a `FinalizationRegistry`: one collected without being released is reported in the console with the file and line that made it. A production build carries no detector.

## Not chosen

- **Reading `lib.dom.d.ts`.** Complete at once, and the path to npm's declarations (phase 5), but its `any`, unions and overloads would bring TypeScript's looseness into Peras's checks.
- **Release by hand only**, with a warning when `release()` is never called: simpler, and leaks stay possible on every path that forgets it.
- **Arrow functions with statements, and closures as values.** A handler is an arrow function whose body is an expression, usually a call: enough for this step, and it keeps handlers from capturing resources.

## Status

**Built.**

- **Checks**: the browser's types and members in `perasc-check` (`browser.rs`, `host.rs`), then a pass over each function of a managed module that follows each resource variable on every path, an `if`'s branches joined, a loop's body given away at most once (`resources.rs`). Every case of E0245 has one error in the UI tests.
- **Generator**: the browser's API in `perasc-codegen-js` (`browser.rs`), and the runtime of resources (`$Resource`, `$on`, `$timer`, `$group`), written into a module only when it uses them.
- **In Chromium**, from Vite's dev server: a managed module builds a page, listens to clicks and typing, draws on a canvas and ticks; released, its listeners are removed and its timer cleared, checked by clicking and typing again; `fetch` reads a file and an error status; `storage` keeps and forgets a value; a listener returned to JavaScript and dropped is reported by the detector with its line. Three runs in a row.
- The documentation's managed examples, the reference's above all, are compiled by the tests, and the reference lists exactly the API the compiler knows.

Not done: more of the browser (forms, `requestAnimationFrame`, WebSockets, workers), arrays of resources, a request as a resource cancelled with its owner, and the leak report in Vite's overlay.

## Consequences

- A web interface can be written in Peras end to end, with its computation in owned modules.
- A handler cannot keep a resource: one that must stop another is written as a function that receives what it needs.
