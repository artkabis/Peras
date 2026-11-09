# Arrays, strings and options

Options, arrays and strings are compiled. Arrays and strings live on the heap, with one owner each: the [memory](memory.md) page has the rules.

## `Array<T>`

A growable list of values of one type, written `[a, b, c]`. Its elements live on the heap, in the module's memory. <span class="status compiled">compiled</span>

```peras build
export fn evens(n: i32): Array<i32> {
  let found: Array<i32> = [];
  for (let i = 0; i < n; i++) {
    if (i % 2 == 0) {
      found.push(i);
    }
  }
  return found;
}

export fn first(xs: Array<i32>): i32 {
  if (xs.length == 0) {
    return -1;
  }
  return xs[0];
}

export fn scale(xs: inout Array<f64>, k: f64): void {
  for (let i = 0; i < xs.length; i++) {
    xs[i] = xs[i] * k;
  }
}
```

| Member       | Gives                                                    |
| ------------ | -------------------------------------------------------- |
| `xs.length`  | the number of elements, an `i32`                         |
| `xs.push(x)` | adds `x` at the end                                      |
| `xs.pop()`   | removes the last element: `Option<T>`, `None` when empty |
| `xs[i]`      | the element at index `i`                                 |
| `xs[i] = v`  | replaces it; `+=` and the others, `++` and `--` work too |
| `xs.copy()`  | a new array with the same elements                       |

**Indices are checked.** An index outside `0` to `xs.length - 1` stops the program with a `RangeError`, before any memory is touched. `push` makes room as needed, doubling it, so that adding elements one by one costs a constant time each, on average.

An array holds any type: numbers, booleans, strings, arrays, structures, options of numbers. An array of structures keeps one buffer per field, its columns: `points[i].x` reads one number from the `x` column, and a loop over `x` alone touches nothing else. An array of strings or of arrays owns them: releasing it releases them, and `copy()` copies them too. Two limits remain: `pop()` on an array of strings or arrays, which would give an option holding one, and `==` between such arrays.

```peras build
struct Point {
  x: f64;
  y: f64;
}

export fn centroid(points: Array<Point>): Point {
  let sx = 0.0;
  let sy = 0.0;
  for (const p of points) {
    sx += p.x;
    sy += p.y;
  }
  const n = points.length.toF64();
  return Point(sx / n, sy / n);
}

export fn shift(points: inout Array<Point>, dx: f64): void {
  for (let i = 0; i < points.length; i++) {
    points[i].x += dx;
  }
}
```

### One owner per array

An array has one owner. `let ys = xs;`, a `return`, or an argument to a `sink` parameter **moves** it: `xs` can no longer be used ([E0204](../errors/E0204.md)) until it is given a new value. To keep both, move a copy, `xs.copy()`. The compiler releases each array when its owner's scope ends; you never write `free`. The rules are on the [memory](memory.md) page.

```peras build
fn total(xs: sink Array<i32>): i32 {
  let sum = 0;
  for (const x of xs) {
    sum += x;
  }
  return sum;
}

export fn twice(): i32 {
  let xs = [1, 2, 3];
  const first = total(xs.copy());
  return first + total(xs);
}
```

### In JavaScript

An array crosses the boundary as a copy: a JavaScript array goes in, a new JavaScript array comes out. An `inout` parameter is copied back into the array passed.

```ts
import { evens, scale } from "./numbers.peras";

evens(7); // [0, 2, 4, 6]
const xs = [1, 2, 3];
scale(xs, 2); // xs is now [2, 4, 6]
```

TypeScript sees `number[]`, `bigint[]` for `i64` and `boolean[]` for `bool`; a parameter the function only reads is `readonly number[]`.

## `String`

Text, in UTF-8, written between double quotes with the usual escapes (`\n`, `\t`, `\"`, `\\`, `\u{1F600}`). A string lives on the heap, like an array, and follows the same rules: one owner, moved by assignment, released by the compiler. <span class="status compiled">compiled</span>

```peras build
export fn greeting(name: String): String {
  return "Hello, " + name;
}

export fn is_empty(s: String): bool {
  return s.length == 0;
}

export fn repeat(s: String, n: i32): String {
  let out = "";
  for (let i = 0; i < n; i++) {
    out += s;
  }
  return out;
}
```

| Operation      | Gives                                |
| -------------- | ------------------------------------ |
| `a + b`        | a new string, `a` then `b`           |
| `s += t`       | adds `t` at the end of `s`, in place |
| `a == b`, `!=` | whether the two hold the same text   |
| `s.length`     | the number of **bytes** in UTF-8     |
| `s.copy()`     | a new string with the same text      |

**`length` counts bytes**, as in Rust and in the memory the string occupies: `"é".length` is 2, an emoji 4. JavaScript's `length` counts UTF-16 units instead; methods over characters come with the [standard library](../reference/roadmap.md#phase-3-a-standard-library).

### Template literals

Between backticks, `${…}` writes a value into the text: a string as it is, a number or a boolean as `toString()` writes it, so as JavaScript does. Any expression fits there, another template included; `` \` `` and `\${` write the characters themselves, and a template can span several lines. <span class="status compiled">compiled</span>

```peras build
export fn label(name: String, count: i64, done: bool, ratio: f64): String {
  return `${name}: ${count} items, done ${done}, ${ratio * 100.0} %`;
}
```

The compiler measures every piece first, then writes them into one buffer: one allocation per template, whatever the number of values. A structure, an array or an option is refused ([E0239](../errors/E0239.md)): the program says which of its fields to write, and how. A template with no `${…}` is an ordinary string literal.

A literal is stored once in the module, and copied into a new buffer where it is used. In JavaScript, a string crosses the boundary as a copy, encoded and decoded as UTF-8; TypeScript sees `string`. An exported function cannot take a string `inout`, since a JavaScript string never changes.

Invisible characters that can reorder what you read, the "Trojan Source" attack, are refused in source code ([E0007](../errors/E0007.md)).

## `Option<T>`

A value that may be absent: `Some(value)` or `None`. Peras has no `null`: absence is in the type, and the compiler makes you handle it. <span class="status compiled">compiled</span>

```peras build
export fn half(n: i32): Option<i32> {
  if (n % 2 != 0) {
    return None;
  }
  return Some(n / 2);
}

export fn quarter(n: i32): Option<i32> {
  const h = half(n)?;
  return half(h);
}

export fn half_or(n: i32, fallback: i32): i32 {
  return half(n).unwrapOr(fallback);
}
```

| Method or operator         | Gives                                                                |
| -------------------------- | -------------------------------------------------------------------- |
| `o.isSome()`, `o.isNone()` | whether a value is present                                           |
| `o.unwrapOr(fallback)`     | the value, or `fallback` when absent; `fallback` is always evaluated |
| `o?`                       | the value; when absent, the function returns `None` at once          |

`?` is allowed in a function that returns an option. Checked arithmetic returns an option too: `a.checkedAdd(b)` is `None` on overflow, where `a + b` would wrap.

```peras build
export fn total(a: i32, b: i32, c: i32): Option<i32> {
  const ab = a.checkedAdd(b)?;
  return ab.checkedAdd(c);
}
```

An option holds any compiled type, a structure included. It costs no allocation: it is a `bool` followed by the value. JavaScript sees the value, or `undefined`:

```ts
import { half } from "./maths.peras";

half(10); // 5
half(7); // undefined
```

The TypeScript declaration says so: `half(n: number): number | undefined`.

> A complete set of methods on strings and arrays (`split`, `slice`, `map`, `filter`…) is planned: see the [roadmap](../reference/roadmap.md).

Next: [memory](memory.md).
