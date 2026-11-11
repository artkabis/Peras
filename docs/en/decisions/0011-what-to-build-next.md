# 0011: What to build next, decided by measurement

_French translation: [../../fr/decisions/0011-what-to-build-next.md](../../fr/decisions/0011-what-to-build-next.md)._

## Context

Managed mode now compiles to JavaScript ([0008](0008-managed-mode.md)), reaches the browser with resources released by their owner ([0009](0009-browser-and-resources.md)), and calls JavaScript and npm with every value checked ([0010](0010-calling-javascript.md)). An assessment of the project asked whether this brings real value over TypeScript, or is naive, and listed several directions without ranking them: arrow functions with statements, the rule against mutable module state, a comparison sort, structures across modules, and more of the browser.

This decision ranks them on measures, not on opinions: two new benchmarks, each with a script that anyone can run again.

## Method

1. **Web application bugs** (`node scripts/bench-web-bugs.mjs`, [bench/web-bugs](../../../bench/web-bugs/RESULTS.md)). Seventeen bugs of web applications, each written as it is naturally written in TypeScript and in a managed Peras module: a missing element, a listener or a timer never stopped, a library that lies about its types, a 64-bit id above 2⁵³, a forgotten `await`, a missing case, an index past the end, an integer total above 2³¹, an alias that sorts the caller's array, the average of an empty list, data without a field. For each, the earliest point where the toolchain stops it: at compile time (`tsc` at its strictest, then typescript-eslint's strict type-checked preset; `perasc check`), at run time by an error, in Chromium, or not at all, the program giving a wrong result or a listener still listening.
2. **A case study** (`node scripts/bench-case-study.mjs`, [bench/case-study](../../../bench/case-study/RESULTS.md)). A table of orders, searched after a delay, sorted by amount, with a summary, removed cleanly: written in TypeScript (99 lines), then in Peras by a developer coming from TypeScript, each refusal of the compiler noted as it came. Then both measured in Chromium, on 1,000 and 10,000 orders.

## Results

### Bugs

| Stopped                     | TypeScript (strict + typescript-eslint) | Peras, before this decision | Peras, after |
| --------------------------- | --------------------------------------: | --------------------------: | -----------: |
| at compile time             |                                       7 |                           8 |            9 |
| at run time, with a message |                                       1 |                           5 |            5 |
| **silent**                  |                                   **8** |                       **3** |        **2** |
| cannot be written           |                                       0 |                           1 |            1 |

TypeScript stops nothing about listeners and timers left running (three cases), nor about a library that lies (two). Peras refuses all five, the first three at compile time, the others at the boundary with the function's name. Peras was behind in three places:

- **A forgotten `await`**: typescript-eslint refuses it, Peras did not. Fixed: [E0248](../errors/E0248.md).
- **An index past the end**: TypeScript refuses it at compile time (`noUncheckedIndexedAccess`); Peras stops it at run time, with the index and the length.
- **A total above 2³¹ in an `i32`**: it wraps silently in Peras, where JavaScript's `number` is exact up to 2⁵³. Still open: see point 5 below.

Two messages were misleading, and are fixed: an element that may be missing was reported as a constant modified, and a resource kept in a module variable as released at once. Both TypeScript and Peras miss an alias that sorts the caller's array.

### The case study

Each refusal met while writing the Peras version, in order:

1. **An arrow function cannot hold statements**: `(e) => { ascending = !ascending; render(); }` is a syntax error. The state moves to module variables, so the table can be shown only once per page.
2. **A parameter cannot take the name of a module variable** (`E0201`), a consequence of 1.
3. **No sort with a comparison**: `sort()` orders numbers only. An insertion sort written by hand replaces it.
4. **An arrow function is not a value** outside the methods that take one.
5. **The search delay cannot be written**: it needs a timer kept between two keystrokes and replaced at each, and a resource can be neither kept in a module variable nor replaced. The delay was dropped.
6. **A structure cannot cross from `.apeiron` to `.peras`** (`E0244`): the computation cannot move to WebAssembly without splitting each order into parallel arrays.
7. **No number formatting**: `toFixed(2)` does not exist, and the page shows `60016.28000000002` where TypeScript shows `60016.28`.
8. **Releasing the group leaves the rows in the page**: listeners are removed, the elements made are not.

Measured in Chromium, two series of medians of 5 fresh pages; Peras's speed as a fraction of TypeScript's:

| Orders | Step  |   TypeScript | Peras, insertion sort | Peras, sort through `extern` |
| -----: | ----- | -----------: | --------------------: | ---------------------------: |
|  1,000 | mount |   4.4–6.6 ms |           0.48 – 0.61 |                  0.75 – 0.90 |
|  1,000 | sort  |   0.6–0.7 ms |           0.40 – 0.50 |                  0.55 – 0.64 |
| 10,000 | mount | 21.1–25.4 ms |       **0.11 – 0.15** |              **0.74 – 0.79** |
| 10,000 | sort  |   4.8–4.9 ms |           0.54 – 0.56 |                  0.67 – 0.79 |

The insertion sort, forced by point 3, makes Peras 7 to 9 times slower at 10,000 orders; given JavaScript's sort through an `extern` block, Peras runs at 0.74 to 0.90 times TypeScript's speed when mounting. The Peras version has 101 lines against 89, and its JavaScript weighs 1,777 bytes gzip against 860, most of it the runtime's helpers.

## Decision

Managed mode keeps its place: on the bugs measured, it lets 2 through silently where TypeScript lets 8, and the five it alone stops are those that cost the most in an application, leaks and lying libraries. Its cost is in writing, not in checking: every one of the eight refusals of the case study is a missing feature, not a rule that caught a bug. The next steps are ordered by how many of them each removes, and by the bugs it closes:

1. **Arrow functions with statements, capturing what they use**, in managed modules: removes points 1, 2 and 4, and lets a component keep its state in its own variables, so that it can be shown more than once. A captured resource stays refused, as now.
2. **Replacing a resource releases the previous one**: `timer = setTimeout(…)` releases the timer it replaces, as an assignment drops a value in Rust. The restarted timer of the bugs benchmark becomes correct by construction, and the search delay can be written (point 5). A module variable still cannot hold a resource.
3. **Sorting by a key**, `xs.sortBy((o) => o.amount)`, numbers and strings, in both modes: removes point 3; measured, 0.11–0.15 of TypeScript's speed at 10,000 rows becomes 0.74–0.79.
4. **`toFixed(digits)`** and a few formats of numbers: point 7, which users see.
5. **Integer overflow stops in debug**, as in Rust: `+`, `-` and `*` on integers throw instead of wrapping in a debug build, and wrap as now in production; `wrappingAdd` and the others still wrap on purpose. It closes the only bug where TypeScript is right and Peras silent. Its cost in WebAssembly loops is measured before it is adopted.
6. **Structures across `.apeiron` and `.peras`**, converted at the boundary as arrays are: point 6, the computation in WebAssembly without parallel arrays.
7. **Elements owned by a group**, removed when it is released: point 8.

Built with this decision, because each closes a measured gap and costs little: [E0248](../errors/E0248.md) for a promise never awaited, a resource in a module variable reported where it is declared, a member that does not exist reported once and as such, and `click()` on elements.

## Not chosen now

- **Reading `.d.ts` files**: the case study did not need it, `extern` blocks were enough.
- **More of the browser typed by hand**: `extern` blocks reach the rest; the case study needed none.
- **Refusing aliases that change a parameter**: both languages miss it, and refusing `let sorted = items` would change how managed modules share values ([0008](0008-managed-mode.md)); to measure again once arrow functions with statements exist.
- **Smaller output**: half the bytes of the case study are the runtime's helpers, written only when used; worth a measure once the features above exist.

## Progress

- **Step 1, done**: arrow functions with statements, capturing what they use, and functions as values in managed modules ([guide](../guide/managed.md#arrow-functions)). The case study keeps its state in the table it mounts: points 1, 2 and 4 are gone, and the benchmark checks that two tables in one page keep their own state, which the version before failed. 97 lines instead of 101, 1,687 bytes gzip instead of 1,777; the bugs benchmark is unchanged, 2 silent.
- **Step 2, done**: replacing the resource of a `let` variable releases the one it held. An arrow function cannot capture a resource, so a resource it must replace is kept in a `Slot` of a group, `group.slot()`: `slot.set(timer)` releases the timer before, and the slot is released with its group, which also holds the listeners that use it. The case study searches after a delay of 50 ms, as the TypeScript version: point 5 is gone. 100 lines, 1,858 bytes gzip, most of the increase the runtime's timers and slots. The restarted timer of the bugs benchmark is still refused at compile time, being in a module variable.
- **Step 3, done**: `sortBy` and `sortByDescending`, in both modes ([methods](../reference/methods.md#sorting-by-a-key)). Measured first: in JavaScript, a radix sort of 8-bit digits over the keys' bits, carrying indices, beat a sort of indices by their keys and TypeScript's comparison from 300 elements; below 256 the native sort is kept. In WebAssembly the runtime sorts the indices, then puts every column in their order. The order is the same in both modes, strings included, and checked against a stable sort of Rust in the interpreter, in WebAssembly and in JavaScript. The case study mounts 10,000 orders at 0.68 to 0.85 of TypeScript's speed instead of 0.13, over four series, the same as with JavaScript's sort through `extern`: point 3 is gone, and what remains of the gap is not the sort. 90 lines against 89; its JavaScript weighs 2,500 bytes gzip, the sort's helpers included; [the sorting benchmark](../../../bench/sort/RESULTS.md) gives 3 to 7 times TypeScript's speed by a number from 10,000 records.

## Consequences

- Each next step has a measure that says whether it worked: the case study is written again with it, and the benchmarks run again. A step that does not move them is not done.
- The roadmap follows this order; the status page and the benchmarks say how far each step is.
