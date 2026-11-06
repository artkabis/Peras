# Getting started

Peras is not published yet: you build the compiler from the repository. You need [rustup](https://rustup.rs), Node 26 and pnpm 12.

## Build the compiler

```sh
git clone https://github.com/artkabis/Peras.git
cd Peras
cargo build --release -p perasc
./target/release/perasc --version
```

`perasc` is a single program: no Rust toolchain, no `wasm-bindgen`, nothing else is needed to compile Peras.

## Your first module

Write `hello.peras`:

```peras build
export fn square(n: i32): i32 {
  return n * n;
}
```

Check it, then build it:

```sh
perasc check hello.peras
perasc build hello.peras --out dist
```

`dist/` now holds three files:

| File         | What it is                                           |
| ------------ | ---------------------------------------------------- |
| `hello.wasm` | the WebAssembly module                               |
| `hello.js`   | an ES module that loads it and exports `square`      |
| `hello.d.ts` | TypeScript declarations: `square(n: number): number` |

```js
import { square } from "./dist/hello.js";

console.log(square(12)); // 144
```

## In a Vite application

The `vite-plugin-peras` plugin imports `.peras` files directly:

```ts
// vite.config.ts
import { defineConfig } from "vite";
import peras from "vite-plugin-peras";

export default defineConfig({
  plugins: [peras({ perasc: "path/to/perasc", lang: "en" })],
});
```

```ts
// main.ts
import { square } from "./hello.peras";

document.body.textContent = String(square(12));
```

During development the module is rebuilt when you save, and an error shows in Vite's overlay with its excerpt, cause and fix. `vite build` fails on any error. The plugin also writes `hello.d.peras.ts` next to your source, so TypeScript knows the real signatures (enable `allowArbitraryExtensions` in your `tsconfig.json`).

The repository's `examples/fib` is a complete application: `pnpm --filter peras-example-fib dev`.

## When something is wrong

```peras error E0203
export fn square(n: i32): i32 {
  let unused = 3;
  return n * n;
}
```

```text
error[E0203]: variable `unused` is never read
  --> hello.peras:2:7
   |
 2 |   let unused = 3;
   |       ^^^^^^ never read
   |
   = why: A value that is never read is dead code, and often a bug: a typo, or a use that was forgotten. A name starting with `_` marks a parameter as unused on purpose.
   = fix: remove it, or use its value
   = details: perasc explain E0203

1 error: the build is blocked
```

Nothing is built until every error is fixed. `--lang fr` shows the same messages in French; `perasc explain E0203` shows the page about this error.

Next: [values and variables](basics.md).
