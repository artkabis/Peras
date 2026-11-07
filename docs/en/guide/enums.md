# Enumerations and match

An enumeration is a value that is one of several variants, each with its own data. `match` takes it apart, and the compiler checks that every variant is handled: forgetting one is an error, not a bug found in production. <span class="status compiled">compiled</span>

## Declaring an enumeration

```peras build
export enum Shape {
  Circle(f64),
  Rect(f64, f64),
  Empty,
}

export fn unit(): Shape {
  return Shape.Rect(1.0, 1.0);
}

export fn nothing(): Shape {
  return Shape.Empty;
}
```

A value is written `Name.Variant(data)`, or `Name.Variant` for a variant without data. The data can be of any type: numbers, strings, arrays, structures, options, other enumerations.

## `match`

```peras build
enum Shape {
  Circle(f64),
  Rect(f64, f64),
  Empty,
}

export fn area(s: Shape): f64 {
  return match (s) {
    Circle(r) => 3.14159 * r * r,
    Rect(w, h) => w * h,
    Empty => 0.0,
  };
}
```

Each arm is a pattern, `=>`, then a value or a block. The arms are tried in order, and the first that matches runs.

| Pattern                        | Matches                                                                       |
| ------------------------------ | ----------------------------------------------------------------------------- |
| `_`                            | anything                                                                      |
| a name, `x`                    | anything, called `x` in the arm; a variant without data, if one has that name |
| `0`, `-1`, `true`              | that number, that boolean                                                     |
| `Circle(r)`, `Shape.Circle(r)` | that variant; `r` names its data                                              |
| `Some(0)`, `Rect(w, _)`        | patterns nest: the variant, when its data matches too                         |

`match` works on enumerations, options, booleans and integers; on any other value, only `_` and names apply.

## Every case, checked

A `match` must cover every value it can receive: each variant of an enumeration or an option, `true` and `false`, and for numbers or strings a final `_` ([E0222](../errors/E0222.md)). Add a variant to `Shape`, and every `match` that forgets it is reported.

```peras error E0222
enum Shape {
  Circle(f64),
  Rect(f64, f64),
  Empty,
}

export fn sides(s: Shape): i32 {
  return match (s) {
    Circle(_) => 0,
    Rect(_, _) => 4,
  };
}
```

An arm that no value can reach, after `_` or after a variant already covered, is an error too ([E0224](../errors/E0224.md)); so is a variant the enumeration does not have ([E0223](../errors/E0223.md)).

## Arms with statements

```peras build
export fn sign(n: i32): i32 {
  match (n) {
    0 => {
      return 0;
    }
    _ => {}
  }
  if (n < 0) {
    return -1;
  }
  return 1;
}
```

A `match` whose arms are blocks is a statement: its `;` is optional, and `return`, `break` and `continue` work inside its arms. When every arm returns, the function needs nothing after it.

## Options

An option is an enumeration with two variants, `None` and `Some(value)`:

```peras build
export fn describe(o: Option<i32>): i32 {
  return match (o) {
    Some(0) => 100,
    Some(x) => x,
    None => -1,
  };
}
```

## Memory

A variant can hold strings and arrays. The value matched is only lent to the arms: a name in a pattern reads its data, and cannot move it ([E0209](../errors/E0209.md)); write `.copy()` to keep it. Inside, an enumeration holds the data of every variant, the unused ones empty, so that it always owns the same buffers: the compiler releases them like any other.

## In JavaScript

An enumeration crosses the boundary as an object, `{ kind: "Circle", values: [1.5] }`, declared to TypeScript as a union: `TypeScript` then checks the `kind` of each case too.

```ts
import { area } from "./shapes.peras";

area({ kind: "Rect", values: [2, 3] }); // 6
```

Next: [arrays, strings and options](collections.md).
