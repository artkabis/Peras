# Peras for decision-makers

**Peras is a language for the parts of a web application where speed and correctness matter most.** It reads like TypeScript. A module written in Peras is either compiled to WebAssembly, with its memory checked by the compiler and no garbage collector, or compiled to plain JavaScript. Both kinds are called from an existing TypeScript application, one function at a time: nothing else in the application changes.

This page gives what a CTO, an architect or a team lead needs to decide whether, where and how to try it. Every figure on it is measured in this repository, and linked to how it is measured.

## Two kinds of modules

| File       | Compiled to | For                                                                                 |
| ---------- | ----------- | ----------------------------------------------------------------------------------- |
| `.peras`   | WebAssembly | computation: data processing, images, simulations, parsing, anything run many times |
| `.apeiron` | JavaScript  | application code that waits and calls the computation: `async`, `await`, `number`   |

Both are the same language. The compiler checks the boundary between them, and between them and TypeScript ([managed modules](managed.md)).

```peras build
export fn average(samples: Array<f64>): f64 {
  return samples.reduce((sum, x) => sum + x, 0.0) / samples.length.toF64();
}
```

```ts
import { average } from "./stats.peras";

average([1, 2, 3, 4]); // 2.5, computed in WebAssembly
```

## What it brings, measured

- **Code written the usual way runs at the speed of code written by an expert.** Against JavaScript as it is usually written, in Chromium: 64-bit integers 20 times faster, sorting 17 times faster than `array.sort((a, b) => a - b)`, text 3.2 to 3.3 times faster than `split` and `filter`, structures 1.5 times faster than an object per item ([browser benchmark](../../../bench/README.md#browser-benchmark-benchbrowser)).
- **Against the fastest JavaScript an expert writes**, it is faster on numbers in arrays (1.6 to 2.4 times), sorting, text and 64-bit integers, and at the same speed on floating-point arithmetic alone. It is slower on very short calls, which cost 1.4 to 1.7 times a JavaScript call: work should cross in large pieces.
- **Every core**: a `parallel for` loop runs on the cores of the machine with the same results, bit for bit, 2.7 to 2.8 times faster on 4 cores on a simulation ([threads](../decisions/0007-threads-over-shared-memory.md)).
- **Against Rust and AssemblyScript**, the two other ways to write WebAssembly for the web, written as they are usually written: within 6 % of Rust or faster (0.94 to 1.52 times its speed), 2.4 to 7.3 times faster than AssemblyScript where it allocates or loops ([comparison](../../../bench/README.md#against-rust-and-assemblyscript)).
- **The JavaScript it writes** (`.apeiron`) runs at the speed of the same JavaScript written by hand: 0.93 to 1.31 times ([managed modules benchmark](../../../bench/README.md#managed-modules-benchmanaged)).

## What it prevents

- **Memory errors**: a block of memory used after it is released, released twice, or never released. The compiler refuses the program, and every module is checked again by a verifier and, in development, by an allocator that catches what slips through ([memory](memory.md)).
- **Silent wrong values**: no implicit conversion, integers that wrap the same way everywhere, an index out of bounds stopped instead of read, every case of a `match` handled. A static type system alone already catches about 15 % of the public bugs of JavaScript projects ([the research behind Peras](../research.md#static-checks-catch-bugs)).
- **Unexplained failures**: every error gives its cause and its fix, in English or French, and has its own page ([error codes](../errors/index.md)).

## Where it fits, and where it does not

| Fits                                                         | Does not fit yet                                                         |
| ------------------------------------------------------------ | ------------------------------------------------------------------------ |
| a function or a module of computation called from TypeScript | a whole application: the browser's APIs are not typed in Peras yet       |
| data that is large, numeric, or processed many times         | code that relies on npm libraries: Peras cannot import them yet          |
| a team with one or two people willing to learn strict rules  | a team that needs the tooling of TypeScript today: no editor support yet |

## Maturity, stated plainly

- **Version 0.0.1.** Until 1.0, a new version may break the language. What is compiled, checked only, or planned is listed feature by feature in [what works today](../reference/status.md).
- **One maintainer**, with an AI assistant; no community yet. The code is open source under the AGPL, with an exception that leaves programs compiled with Peras to their authors ([licensing](../decisions/0006-licensing.md)).
- **Tested on every change**: every example of this documentation is compiled by the tests; the results of Peras are compared with JavaScript, Rust and the compiler's own interpreter; the end-to-end tests check the modules for memory leaks. The risks found by an outside review are listed in [before using Peras in production](../reference/roadmap.md#before-using-peras-in-production).

## How to try it safely

1. **Pick one isolated module** where speed matters: a filter, a parser, a computation run on every frame. Keep the rest of the application in TypeScript.
2. **Give it to one or two volunteers** interested in performance. Research on learning languages finds that what a developer already knows gets in the way of a new language that looks familiar ([the research behind Peras](../research.md#adoption-and-its-risks)): this is easier to absorb in a small team.
3. **Measure before and after**, with the benchmarks of this repository as a model. Keep Peras where the measure shows a gain.
4. **Read the decisions**: each choice of the language is written down, with what was measured and what was set aside ([decisions](../decisions/0001-webassembly-backend.md), [research](../research.md)).

## Where to go next

- **Engineers**: [getting started](getting-started.md), then [values and variables](basics.md).
- **Architects**: [Peras and JavaScript](javascript.md), [managed modules](managed.md), [architecture](../architecture.md).
- **Managers and auditors**: [what works today](../reference/status.md), [roadmap](../reference/roadmap.md), [the research behind Peras](../research.md).
