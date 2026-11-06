# Arrays, strings and options

These three types are part of the language and fully checked, but not compiled yet: `perasc check` accepts them, `perasc build` reports [E0900](../errors/E0900.md). Arrays come next, with the heap.

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

A value that may be absent: `Some(value)` or `None`. Peras has no `null`: absence is in the type, and the compiler makes you handle it.

```peras
export fn last_or(xs: Array<i32>, fallback: i32): i32 {
  let copy = xs;
  return copy.pop().unwrapOr(fallback);
}

export fn half(n: i32): Option<i32> {
  if (n % 2 != 0) {
    return None;
  }
  return Some(n / 2);
}
```

| Method                     | Gives                                |
| -------------------------- | ------------------------------------ |
| `o.isSome()`, `o.isNone()` | whether a value is present           |
| `o.unwrapOr(fallback)`     | the value, or `fallback` when absent |

Checked arithmetic returns an option too: `a.checkedAdd(b)` is `None` on overflow.

> A complete set of methods on strings and arrays (`split`, `slice`, `map`, `filter`…) is planned: see the [roadmap](../reference/roadmap.md).

Next: [memory](memory.md).
