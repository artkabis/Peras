# Peras

**A web language with TypeScript's syntax and Rust's rigour.** Peras compiles `.peras` modules to WebAssembly with exact memory, no garbage collector, and refuses to build as long as anything is wrong, explaining the cause and the fix in English or French.

```peras build
export struct Vec2 {
  x: f64;
  y: f64;
}

export fn length(v: Vec2): f64 {
  return (v.x * v.x + v.y * v.y).sqrt();
}
```

```ts
import { length } from "./geometry.peras";

length({ x: 3, y: 4 }); // 5, computed by WebAssembly
```

## Where to start

- [Peras for decision-makers](guide/overview.md): what it brings, measured, where it fits, how mature it is, and how to try it safely.
- [Introduction](guide/introduction.md): what Peras is for, its two modes, and what makes it different.
- [Getting started](guide/getting-started.md): build the compiler, write a module, run it in the browser with Vite.
- [The language, step by step](guide/basics.md): values, functions, control flow, structures, collections, memory.
- [What works today](reference/status.md): every feature, compiled, checked only, or planned. Peras is young: this page says exactly how young.

## Reference

- [Types](reference/types.md), [operators](reference/operators.md), [built-in methods](reference/methods.md), [keywords](reference/keywords.md).
- [The `perasc` command](reference/cli.md).
- [Error codes](errors/index.md): one page per error, also shown by `perasc explain`.
- [Toward a complete language](reference/roadmap.md): what comes next, and in which order.

## The project

- [Language spec](spec.md): the language as designed, including what is not built yet.
- [Kickoff plan](kickoff-plan.md) and [decisions](decisions/0001-webassembly-backend.md): how the compiler is built and why.

Every code block of this documentation is compiled by `perasc` in the project's tests: one that says it compiles does, and one that says it is refused is, with the code shown.
