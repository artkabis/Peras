# Memory

In owned mode, Peras manages memory without a garbage collector and without you writing `free`: every value has one owner, and is released when its owner's scope ends.

> This page describes the model. Numbers, booleans and structures need none of it: they live in WebAssembly's locals, not on the heap. The ownership checks arrive with arrays, the first type on the heap; the compiler's foundations for them are already built and tested (see below).

## One owner per value

- Each value has a single owner: a variable, a field, or a parameter.
- Assigning a value that lives on the heap **moves** it: the old name can no longer be used.
- No reference is ever stored inside a structure or an array, so there is never a lifetime to write, unlike Rust.
- Sharing is explicit, through `Rc<T>`, visible in the type.

## Three ways to pass a value

| Convention       | The function…              | The caller…                                      |
| ---------------- | -------------------------- | ------------------------------------------------ |
| `x: T` (default) | reads `x`                  | keeps it, unchanged                              |
| `x: inout T`     | may modify `x` in place    | writes `&value`; no alias exists during the call |
| `x: sink T`      | takes `x`, and releases it | can no longer use it                             |

```peras
struct Buffer {
  samples: Array<f32>;
}

fn gain(buf: inout Buffer, factor: f32): void {
  for (let i = 0; i < buf.samples.length; i++) {
    buf.samples[i] = buf.samples[i] * factor;
  }
}

fn consume(buf: sink Buffer): i32 {
  return buf.samples.length;
}

export fn process(data: Array<f32>): i32 {
  let buf = Buffer(data);
  gain(&buf, 0.8);
  return consume(buf);
}
```

## Where values live

| Value             | Place                                        |
| ----------------- | -------------------------------------------- |
| numbers, `bool`   | WebAssembly locals: no memory address at all |
| structures        | their fields, as locals                      |
| `Array`, `String` | the heap, in the module's linear memory      |

## What protects you

Releasing memory by hand is where C programs go wrong: releasing twice, using memory after releasing it, forgetting to release. In Peras the compiler inserts every release, and three checks guard the compiler itself:

1. **A release verifier.** Before any code is generated, it proves that every heap value is released exactly once on every path: `return`, `break`, `continue` included. A program that would release twice or leak is not compiled; it is a bug in `perasc`, reported as such.
2. **A debug allocator.** In development, released memory is poisoned and kept in quarantine, every block has a red zone, and leaks are counted. A memory bug throws a `PerasMemoryError` that names where the block was allocated and released:

   ```text
   PerasMemoryError: the block allocated at parse (alloc 1) is released a second time, at parse (release 2)
   ```

3. **A differential test.** Two independent code generators must agree on results and on the number of blocks still alive.

The details, and the measurements, are in [decision 0001](../decisions/0001-webassembly-backend.md).

Next: [reading diagnostics](diagnostics.md).
