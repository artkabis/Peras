# Built-in methods

Every method the language provides, by type. This page is checked against the compiler: a method missing here, or listed here but unknown to the compiler, fails the tests.

## Numbers

### Conversions

| Method                                    | On           | Gives                           | Status                                        |
| ----------------------------------------- | ------------ | ------------------------------- | --------------------------------------------- |
| `toI32()`, `toI64()`                      | every number | the value as that integer type  | <span class="status compiled">compiled</span> |
| `toF32()`, `toF64()`                      | every number | the value as that float type    | <span class="status compiled">compiled</span> |
| `toI8()`, `toI16()`                       | every number | the value as that integer type  | <span class="status checked">checked</span>   |
| `toU8()`, `toU16()`, `toU32()`, `toU64()` | every number | the value as that unsigned type | <span class="status checked">checked</span>   |

From a float, conversions round toward zero and saturate; between integers, they keep the low bits.

### Arithmetic

| Method                                               | On           | Gives                                              | Status                                        |
| ---------------------------------------------------- | ------------ | -------------------------------------------------- | --------------------------------------------- |
| `abs()`                                              | every number | the absolute value; wraps for the smallest integer | <span class="status compiled">compiled</span> |
| `wrappingAdd(b)`, `wrappingSub(b)`, `wrappingMul(b)` | integers     | the result wrapped around, as `+`, `-`, `*`        | <span class="status compiled">compiled</span> |
| `checkedAdd(b)`, `checkedSub(b)`, `checkedMul(b)`    | integers     | `Some(result)`, or `None` on overflow              | <span class="status compiled">compiled</span> |
| `min(b)`, `max(b)`                                   | every number | the smaller, the larger                            | <span class="status compiled">compiled</span> |
| `clamp(lo, hi)`                                      | every number | the value, raised to `lo`, lowered to `hi`         | <span class="status compiled">compiled</span> |

```peras build
export fn wrapped(a: i32): i32 {
  return a.wrappingMul(3).abs();
}
```

### As text

| Method       | On                     | Gives                                                                      | Status                                        |
| ------------ | ---------------------- | -------------------------------------------------------------------------- | --------------------------------------------- |
| `toString()` | integers, and booleans | the decimal digits, after `-` when negative; `true` or `false`, a `String` | <span class="status compiled">compiled</span> |

The text is what JavaScript's `String()` writes for the same value. Floats come next: their shortest exact form needs an algorithm of its own.

```peras build
export fn label(count: i64, done: bool): String {
  return count.toString() + " items, done: " + done.toString();
}
```

### Floats

| Method                    | Gives                                                | Status                                        |
| ------------------------- | ---------------------------------------------------- | --------------------------------------------- |
| `sqrt()`                  | the square root; NaN below zero                      | <span class="status compiled">compiled</span> |
| `floor()`, `ceil()`       | rounded down, up                                     | <span class="status compiled">compiled</span> |
| `round()`                 | rounded to the nearest, halves up, as `Math.round`   | <span class="status compiled">compiled</span> |
| `sin()`, `cos()`, `tan()` | trigonometry, in radians                             | <span class="status compiled">compiled</span> |
| `exp()`, `log()`          | `e` to the power of the value, the natural logarithm | <span class="status compiled">compiled</span> |
| `pow(b)`                  | the value to the power `b`                           | <span class="status compiled">compiled</span> |
| `atan2(x)`                | on `y`, the angle of the point `(x, y)`, in radians  | <span class="status compiled">compiled</span> |
| `hypot(b)`                | `sqrt(a * a + b * b)`                                | <span class="status compiled">compiled</span> |

WebAssembly has no instruction for `round`, `exp`, `log`, the trigonometry, `pow` and `atan2`: a module that uses them imports them from JavaScript's `Math`, so they give exactly what JavaScript gives. An `f32` is computed as an `f64`, then rounded back.

## `Array<T>`

| Member    | Gives                                                 | Status                                        |
| --------- | ----------------------------------------------------- | --------------------------------------------- |
| `length`  | the number of elements, an `i32`                      | <span class="status compiled">compiled</span> |
| `push(x)` | adds `x` at the end                                   | <span class="status compiled">compiled</span> |
| `pop()`   | removes and gives the last element, `None` when empty | <span class="status compiled">compiled</span> |
| `copy()`  | a new array with the same elements                    | <span class="status compiled">compiled</span> |

### Order, parts and search

| Method              | Gives                                                                                                 | Status                                        |
| ------------------- | ----------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `slice(start, end)` | a new array of the elements from `start` to `end`; negative counts from the end, beyond is clamped    | <span class="status compiled">compiled</span> |
| `concat(other)`     | a new array of its elements, then those of `other`                                                    | <span class="status compiled">compiled</span> |
| `reverse()`         | reverses the elements, in place: the array is a `let`                                                 | <span class="status compiled">compiled</span> |
| `sort()`            | sorts numbers in increasing order, in place, as typed arrays do in JavaScript: -0 before +0, NaN last | <span class="status compiled">compiled</span> |
| `includes(x)`       | whether an element equals `x`; numbers, booleans and strings                                          | <span class="status compiled">compiled</span> |
| `indexOf(x)`        | the index of the first element equal to `x`, or `-1`                                                  | <span class="status compiled">compiled</span> |

```peras build
export fn median(samples: Array<f64>): f64 {
  let sorted = samples.copy();
  sorted.sort();
  return sorted[sorted.length / 2];
}
```

`sort()` orders numbers by their bits, a radix sort: its time grows with the length, not with length × log(length), and a million `f64` take about 30 ms in a browser. It uses a scratch copy of the array, given back before it returns.

### With an arrow function

Each of these methods takes an arrow function, written in place, `(x) => …`, and compiles it as the body of a loop: no function value and no call at run time. Its parameters lend the elements, as `for…of` does: they are read, never moved. <span class="status compiled">compiled</span>

| Method                | Gives                                                                                                                                                      | Status                                        |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `map(f)`              | a new array of `f(x)` for each element                                                                                                                     | <span class="status compiled">compiled</span> |
| `filter(f)`           | a new array of copies of the elements for which `f(x)` is `true`; on a temporary array, as `text.split(" ")`, the same array, kept in place without copies | <span class="status compiled">compiled</span> |
| `reduce(f, start)`    | `f(acc, x)` folded over the elements, from `start`; numbers and structures of them                                                                         | <span class="status compiled">compiled</span> |
| `find(f)`             | a copy of the first element for which `f(x)` is `true`, or `None`                                                                                          | <span class="status compiled">compiled</span> |
| `findIndex(f)`        | the index of the first element for which `f(x)` is `true`, or `-1`                                                                                         | <span class="status compiled">compiled</span> |
| `some(f)`, `every(f)` | whether `f(x)` is `true` for one element, for all; stops as soon as known                                                                                  | <span class="status compiled">compiled</span> |

```peras build
export fn summary(xs: Array<i32>, words: Array<String>): i32 {
  const evens = xs.filter((x) => x % 2 == 0);
  const total = evens.map((x) => x * 10).reduce((acc, x) => acc + x, 0);
  const long = words.find((w) => w.length > 3).unwrapOr("");
  let n = total + long.length + xs.findIndex((x) => x > 100);
  if (xs.some((x) => x < 0)) {
    n += 1;
  }
  return n;
}
```

## `String`

| Member   | Gives                           | Status                                        |
| -------- | ------------------------------- | --------------------------------------------- |
| `length` | the length                      | <span class="status compiled">compiled</span> |
| `copy()` | a new string with the same text | <span class="status compiled">compiled</span> |

Indices and lengths count bytes of UTF-8, as `length` does. An index inside a character moves back to its start, so a slice is always valid text. Every method that gives a string gives a new one; the string it is called on does not change.

| Method                                        | Gives                                                                                                                                                                      | Status                                        |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `indexOf(s)`                                  | the byte index of the first `s`, or `-1`                                                                                                                                   | <span class="status compiled">compiled</span> |
| `includes(s)`, `startsWith(s)`, `endsWith(s)` | whether `s` is in it, at its start, at its end                                                                                                                             | <span class="status compiled">compiled</span> |
| `slice(start, end)`                           | the bytes from `start` to `end`; negative counts from the end, beyond the ends is clamped                                                                                  | <span class="status compiled">compiled</span> |
| `trim()`                                      | without the spaces, tabs and line breaks at both ends                                                                                                                      | <span class="status compiled">compiled</span> |
| `toUpperCase()`, `toLowerCase()`              | ASCII letters in upper or lower case; other characters unchanged                                                                                                           | <span class="status compiled">compiled</span> |
| `repeat(n)`                                   | the text `n` times                                                                                                                                                         | <span class="status compiled">compiled</span> |
| `replace(from, to)`, `replaceAll(from, to)`   | the first `from`, or every one, replaced by `to`                                                                                                                           | <span class="status compiled">compiled</span> |
| `split(separator)`                            | the parts between each separator, an `Array<String>`; with `""`, the characters; read at once, as `text.split(" ").filter(…)`, the parts are views of the text, not copies | <span class="status compiled">compiled</span> |
| `parseI32()`, `parseI64()`                    | `Some(n)` when the text is an integer in range, else `None`; see below                                                                                                     | <span class="status compiled">compiled</span> |

```peras build
export fn tidy(line: String): String {
  const fields = line.split(",").map((f) => f.trim().toLowerCase());
  const named = fields.filter((f) => !f.startsWith("#"));
  return named.join(" ").replaceAll("  ", " ");
}
```

`parseI32()` and `parseI64()` are strict: the text is a `+` or `-` or neither, then ASCII digits, and nothing else. No spaces, no `_`, no `0x`, no exponent; a value outside the type's range is `None`, never wrapped. Trim first if the text may carry spaces.

```peras build
export fn port(text: String): i32 {
  const n = text.trim().parseI32().unwrapOr(-1);
  return n.clamp(-1, 65535);
}
```

On an `Array<String>`, `join(separator)` gives its strings with `separator` between each two. <span class="status compiled">compiled</span>

## `Option<T>`

| Method                 | Gives                      | Status                                        |
| ---------------------- | -------------------------- | --------------------------------------------- |
| `isSome()`, `isNone()` | whether a value is present | <span class="status compiled">compiled</span> |
| `unwrapOr(fallback)`   | the value, or `fallback`   | <span class="status compiled">compiled</span> |

## `Result<T, E>`

| Method               | Gives                         | Status                                        |
| -------------------- | ----------------------------- | --------------------------------------------- |
| `isOk()`, `isErr()`  | which variant it is           | <span class="status compiled">compiled</span> |
| `unwrapOr(fallback)` | the `Ok` value, or `fallback` | <span class="status compiled">compiled</span> |

An unknown member is refused with the list of the ones that exist ([E0215](../errors/E0215.md)). Many more methods are planned: see the [roadmap](roadmap.md#phase-3-a-standard-library).
