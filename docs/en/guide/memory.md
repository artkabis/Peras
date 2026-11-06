# Memory

In owned mode, Peras manages memory without a garbage collector and without you writing `free`: every value has one owner, and is released when its owner's scope ends.

> Numbers, booleans, structures of them and options need none of this: they live in WebAssembly's locals, not on the heap. Arrays live on the heap, and the compiler applies everything on this page to them. Strings come next, with the same rules.

## One owner per value

- Each value on the heap has a single owner: a variable, a field, or a `sink` parameter.
- Reading a value borrows it. Assigning it, returning it, or passing it to a `sink` parameter, a structure or `push` **moves** it: the old name holds nothing until it is given a new value ([E0204](../errors/E0204.md)).
- Only the owner can move a value: a parameter that is not `sink`, a field or an element is only borrowed ([E0209](../errors/E0209.md)). Move a copy, `xs.copy()`, to keep both.
- No reference is ever stored inside a structure or an array, so there is never a lifetime to write, unlike Rust.
- Sharing will be explicit, through `Rc<T>`, visible in the type.

The checks follow every path: a value moved on one side of an `if` counts as moved after it; in a loop, a value moved in one turn is moved in the next one, unless the turn gives it a new value or leaves with `break`.

## Three ways to pass a value

| Convention       | The function…              | The caller…                                      |
| ---------------- | -------------------------- | ------------------------------------------------ |
| `x: T` (default) | reads `x`                  | keeps it, unchanged                              |
| `x: inout T`     | may modify `x` in place    | writes `&value`; no alias exists during the call |
| `x: sink T`      | takes `x`, and releases it | can no longer use it                             |

`&` marks an `inout` argument, and only it ([E0206](../errors/E0206.md)). No other argument of the call may reach the same variable ([E0207](../errors/E0207.md)), and a `for…of` loop does not let its array change size.

```peras build
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

export fn process(data: sink Array<f32>): i32 {
  let buf = Buffer(data);
  gain(&buf, 0.8);
  return consume(buf);
}
```

## When memory is released

The compiler inserts every release; you never write one:

- at the end of the block that declares a variable, if it still owns its value;
- when a variable receives a new value, for the old one;
- before `return`, for everything the function still owns; before `break` and `continue`, for what the loop's body declared;
- where paths meet: a value moved on one of them is released on the others, at once.

```peras build
fn total(xs: sink Array<i32>): i32 {
  return xs.length;
}

export fn count(n: i32): i32 {
  let xs: Array<i32> = [];
  let sum = 0;
  for (let i = 0; i < n; i++) {
    xs.push(i);
    if (i == 10) {
      sum += total(xs);
      break;
    }
  }
  return sum;
}
```

Here `xs` is released by `total` on the path that breaks, and as soon as the loop ends on the path where it runs to its end.

## Where values live

| Value                       | Place                                        |
| --------------------------- | -------------------------------------------- |
| numbers, `bool`             | WebAssembly locals: no memory address at all |
| structures, options         | their fields, as locals                      |
| `Array` (and soon `String`) | the heap, in the module's linear memory      |

## What protects you

Releasing memory by hand is where C programs go wrong: releasing twice, using memory after releasing it, forgetting to release. In Peras the compiler inserts every release, and three checks guard the compiler itself:

1. **A release verifier.** Before any code is generated, it proves that every heap value is released exactly once on every path: `return`, `break`, `continue` included. A program that would release twice or leak is not compiled; it is a bug in `perasc`, reported as such.
2. **A debug allocator.** In development, released memory is poisoned and kept in quarantine, every block has a red zone, and leaks are counted. A memory bug throws a `PerasMemoryError` that names where the block was allocated and released:

   ```text
   PerasMemoryError: the block allocated at parse (alloc 1) is released a second time, at parse (release 2)
   ```

3. **Tests on every path.** The test programs run in an interpreter of the compiler's intermediate code, and as WebAssembly on the debug allocator: both must give the expected results and leave no block alive.

The details, and the measurements, are in [decision 0001](../decisions/0001-webassembly-backend.md).

Next: [reading diagnostics](diagnostics.md).
