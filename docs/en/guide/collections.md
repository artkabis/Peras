# Arrays, strings and options

Options are compiled. Arrays and strings are part of the language and fully checked, but not compiled yet: `perasc check` accepts them, `perasc build` reports [E0900](../errors/E0900.md). They come next, with the heap.

## `Array<T>`

A growable list of values of one type, written `[a, b, c]`:

```peras
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
```

| Member       | Gives                                                    |
| ------------ | -------------------------------------------------------- |
| `xs.length`  | the number of elements, an `i32`                         |
| `xs.push(x)` | adds `x` at the end                                      |
| `xs.pop()`   | removes the last element: `Option<T>`, `None` when empty |
| `xs[i]`      | the element at index `i`                                 |

## `String`

Text, in UTF-8, written between double quotes with the usual escapes (`\n`, `\t`, `\"`, `\\`, `\u{1F600}`):

```peras
export fn greeting(name: String): String {
  return "Hello, " + name;
}

export fn is_empty(s: String): bool {
  return s.length == 0;
}
```

`+` joins two strings, `==` and `!=` compare them, `s.length` gives the length. Whether a length counts bytes or characters is decided when strings compile.

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

> `pop()` on an array returns an option; it compiles with arrays.

> A complete set of methods on strings and arrays (`split`, `slice`, `map`, `filter`…) is planned: see the [roadmap](../reference/roadmap.md).

Next: [memory](memory.md).
