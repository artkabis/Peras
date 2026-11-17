# Memory

In owned mode, Peras manages memory without a garbage collector and without you writing `free`: every value has one owner, and is released when its owner's scope ends.

> Numbers, booleans, structures of them and options need none of this: they live in WebAssembly's locals, not on the heap. Arrays and strings live on the heap, and the compiler applies everything on this page to them.

## One owner per value

- Each value on the heap has a single owner: a variable, a field, or a `sink` parameter.
- Reading a value borrows it. Assigning it, returning it, or passing it to a `sink` parameter, a structure or `push` **moves** it: the old name holds nothing until it is given a new value ([E0204](../errors/E0204.md)).
- A move into a call happens at the call, once every argument is evaluated: a later argument may read what an earlier one moves, `m.set(key, m.get(key).unwrapOr(0) + 1)`. Lending it to the same call as well stays refused, since the call would hold it twice ([decision 0028](../decisions/0028-options-and-moves.md)).
- Only the owner can move a value: a parameter that is not `sink`, an element, or a field of a structure the function does not own is only borrowed ([E0209](../errors/E0209.md)). Move a copy, `xs.copy()`, to keep both.
- A field moves out of a structure the function owns, a variable or a `sink` parameter: `const country = row.country;`. The structure's other fields stay usable; the structure itself, or the field, cannot be used again until the field is given a new value ([E0204](../errors/E0204.md), [decision 0025](../decisions/0025-moved-fields-and-function-values.md)).
- No reference is ever stored inside a structure or an array, so there is never a lifetime to write, unlike Rust.
- Sharing is explicit, through `Rc<T>`, visible in the type: see [Sharing a value](#sharing-a-value).

The checks follow every path: a value moved on one side of an `if` counts as moved after it; in a loop, a value moved in one turn is moved in the next one, unless the turn gives it a new value or leaves with `break`.

## Three ways to pass a value

| Convention       | The function…              | The caller…                                      |
| ---------------- | -------------------------- | ------------------------------------------------ |
| `x: T` (default) | reads `x`                  | keeps it, unchanged                              |
| `x: inout T`     | may modify `x` in place    | writes `&value`; no alias exists during the call |
| `x: sink T`      | takes `x`, and releases it | can no longer use it                             |

`&` marks an `inout` argument, and only it ([E0206](../errors/E0206.md)). No other argument of the call may reach the same place, and nothing else in the statement may read it ([E0207](../errors/E0207.md)); two different fields of one structure are different places. A `for…of` loop does not let its array change size.

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

## Sharing a value

`Rc.new(value)` makes a value many holders can hold: copying an `Rc<T>` holds the same value once more, without copying it, and the last holder to end releases it. A type can hold itself through one, a tree or a list ([decision 0032](../decisions/0032-shared-values.md)).

```peras build
struct Mesh {
  tris: Array<i32>;
}

struct Lantern {
  mesh: Rc<Mesh>;
  x: f64;
}

struct Node {
  value: i32;
  left: Option<Rc<Node>>;
  right: Option<Rc<Node>>;
}

export fn triangles(count: i32): i32 {
  const mesh = Rc.new(Mesh([0, 1, 2]));
  let lanterns: Array<Lantern> = [];
  for (let i = 0; i < count; i++) {
    lanterns.push(Lantern(mesh, i.toF64()));
  }
  let total = 0;
  for (const lantern of lanterns) {
    total += lantern.mesh.tris.length / 3;
  }
  return total;
}

fn sum(node: Option<Rc<Node>>): i32 {
  if (node.isNone()) {
    return 0;
  }
  return node.value + sum(node.left) + sum(node.right);
}

export fn small(): i32 {
  const leaf = Some(Rc.new(Node(1, None, None)));
  return sum(Some(Rc.new(Node(2, leaf, None))));
}
```

- **A shared value stays a value.** Writing through one holder, `b.tris.push(3)` or `b.x = 1`, first makes the value that holder's own: in place when no one else holds it, on a copy otherwise. The other holders keep what they had.
- **No cycle can be built**, so nothing is ever left alive: a value can only hold values made before it. A child cannot hold the parent it is added to; a graph that points back is written from the parent, or by numbers in an array.
- **Reading costs nothing more.** `const mesh = lanterns[i].mesh` is lent, not held once more, while nothing changes `lanterns` or `i`. A turn of `parallel for` may read a shared value, not hold it once more nor write it ([E0241](../errors/E0241.md)).
- **A managed module shares values as an owned one does**: its `Rc` is a box that counts its holders, copied at the first write through one of several ([decision 0033](../decisions/0033-shared-values-in-managed-modules.md)); the same program gives the same values in both modes.
- **For speed, numbers still win.** A tree of a million shared nodes takes 6.8 times as long as the same tree kept in arrays, each child its index, and 4.75 times the memory.

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

| Value               | Place                                        |
| ------------------- | -------------------------------------------- |
| numbers, `bool`     | WebAssembly locals: no memory address at all |
| structures, options | their fields, as locals                      |
| `Array`, `String`   | the heap, in the module's linear memory      |

## What protects you

Releasing memory by hand is where C programs go wrong: releasing twice, using memory after releasing it, forgetting to release. In Peras the compiler inserts every release, and three checks guard the compiler itself:

1. **A release verifier.** Before any code is generated, it proves that every heap value is released exactly once on every path: `return`, `break`, `continue` included. A program that would release twice or leak is not compiled; it is a bug in `perasc`, reported as such.
2. **A debug allocator.** In development, released memory is poisoned and kept in quarantine, every block has a red zone, and leaks are counted. A memory bug throws a `PerasMemoryError` that names where the block was allocated and released:

   ```text
   PerasMemoryError: the block allocated at parse (alloc 1) is released a second time, at parse (release 2)
   ```

3. **Tests on every path.** The test programs run in an interpreter of the compiler's intermediate code, and as WebAssembly on the debug allocator: both must give the expected results and leave no block alive.

In production, `vite build` and `perasc build --allocator tlsf` link a TLSF allocator instead: every allocation and release in bounded time, freed blocks merged at once, 2.7 times faster on a program that allocates a lot ([decision 0002](../decisions/0002-tlsf-allocator.md)).

The details, and the measurements, are in [decision 0001](../decisions/0001-webassembly-backend.md).

Next: [reading diagnostics](diagnostics.md).
