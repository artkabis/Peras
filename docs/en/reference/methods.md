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

```peras build
export fn wrapped(a: i32): i32 {
  return a.wrappingMul(3).abs();
}
```

### Floats

| Method                    | Gives                           | Status                                        |
| ------------------------- | ------------------------------- | --------------------------------------------- |
| `sqrt()`                  | the square root; NaN below zero | <span class="status compiled">compiled</span> |
| `floor()`, `ceil()`       | rounded down, up                | <span class="status compiled">compiled</span> |
| `round()`                 | rounded to the nearest          | <span class="status checked">checked</span>   |
| `sin()`, `cos()`, `tan()` | trigonometry, in radians        | <span class="status checked">checked</span>   |

## `Array<T>`

| Member    | Gives                                                 | Status                                        |
| --------- | ----------------------------------------------------- | --------------------------------------------- |
| `length`  | the number of elements, an `i32`                      | <span class="status compiled">compiled</span> |
| `push(x)` | adds `x` at the end                                   | <span class="status compiled">compiled</span> |
| `pop()`   | removes and gives the last element, `None` when empty | <span class="status compiled">compiled</span> |
| `copy()`  | a new array with the same elements                    | <span class="status compiled">compiled</span> |

## `String`

| Member   | Gives                           | Status                                        |
| -------- | ------------------------------- | --------------------------------------------- |
| `length` | the length                      | <span class="status compiled">compiled</span> |
| `copy()` | a new string with the same text | <span class="status compiled">compiled</span> |

## `Option<T>`

| Method                 | Gives                      | Status                                        |
| ---------------------- | -------------------------- | --------------------------------------------- |
| `isSome()`, `isNone()` | whether a value is present | <span class="status compiled">compiled</span> |
| `unwrapOr(fallback)`   | the value, or `fallback`   | <span class="status compiled">compiled</span> |

An unknown member is refused with the list of the ones that exist ([E0215](../errors/E0215.md)). Many more methods are planned: see the [roadmap](roadmap.md#phase-3-a-standard-library).
