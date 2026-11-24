# Tests

A test is written in the file it tests, next to the code, in Peras ([decision 0040](../decisions/0040-tests.md)):

```peras build
fn digit(c: String): i32 {
  return "0123456789".indexOf(c);
}

export fn parsePort(text: String): Result<u16, String> {
  if (text.length == 0) {
    return Err("empty");
  }
  let n = 0;
  for (const c of text.split("")) {
    const d = digit(c);
    if (d < 0) {
      return Err(`not a digit: ${c}`);
    }
    n = n * 10 + d;
    if (n > 65535) {
      return Err("too large");
    }
  }
  return Ok(n.toU16());
}

test "a port is read" {
  expect(parsePort("80").unwrapOr(0)).toBe(80);
  expect(parsePort("65535")).toBeOk();
}

test "a bad text is refused" {
  expect(parsePort("65536")).toBeErr();
  expect(parsePort("")).toBeErr();
}

test "a digit is found" {
  expect(digit("7")).toBe(7);
  assert(digit("x") < 0);
}
```

A `test "name" { … }` block reaches every function of its file, exported or not: `digit` is tested though JavaScript never sees it.

## What a test says

- `expect(a).toBe(b)` and `expect(a).toEqual(b)` fail when `a != b`, saying where and both values: `port.peras:24: expected 80, got 0`.
- `expect(r).toBeOk()` and `expect(r).toBeErr()` test a `Result`.
- `assert(cond)` fails when `cond` is false, and `assertEqual(a, b)` when `a != b`, saying where and what was asserted.

A test stops at its first failure. A value a template literal cannot write yet, an array or a structure, cannot be shown in a failure: compare it, `expect(xs == [1, 2]).toBe(true)`, until it can (finding F227).

## Running them

`vitest-peras` runs the tests of `.peras` and `.apeiron` files with Vitest:

```ts
// vitest.config.ts
import perasTests from "vitest-peras";

export default {
  plugins: [perasTests()],
  test: { include: ["src/**/*.peras", "src/**/*.apeiron"] },
};
```

Each block is a test of Vitest, run in the module's own mode: an owned module's tests run in WebAssembly, on the debug allocator, which catches every overflow and every leak; a managed module's in JavaScript. `perasc build --tests` is what the plugin calls: each block an exported function, the names of the tests next to the module.

## What they cost

Nothing to the module that ships: a build without `--tests` reads the file as if its tests were not there, and gives the same bytes. `perasc check` reads them the same way, so an error inside a test is reported by the test build, not by `check`.
