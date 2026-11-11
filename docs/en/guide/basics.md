# Values and variables

## Variables

`let` declares a variable you can change, `const` one you cannot. The type comes from the value, or is written after a colon.

```peras build
export fn area(width: f64, height: f64): f64 {
  const scale = 2.0;
  let total: f64 = width * height;
  total = total * scale;
  return total;
}
```

Peras is strict about names, to keep code readable:

- a name is declared once per function: no shadowing ([E0202](../errors/E0202.md));
- every variable is read at least once ([E0203](../errors/E0203.md)); prefix it with `_` to keep it on purpose;
- a `const` is never assigned ([E0205](../errors/E0205.md)).

```peras error E0202
export fn twice(a: i32): i32 {
  let a = a * 2;
  return a;
}
```

## Module constants

A `const` outside any function is a module constant. Its value is known when compiling: literals, other constants, operators, arrays, tuples, structures, options and variants ([E0233](../errors/E0233.md)). It is written into the code wherever it is used, so a constant array or string is a new value at each use. <span class="status compiled">compiled</span>

```peras build
const SIDE: i32 = 4;
const AREA: i32 = SIDE * SIDE;
const LABEL: String = "square";

export fn describe(): i32 {
  return AREA + LABEL.length;
}
```

A module `let`, a variable shared by all functions, is checked but not compiled yet. <span class="status checked">checked</span>

## Numbers

| Type                      | Values                 |
| ------------------------- | ---------------------- |
| `i8`, `i16`, `i32`, `i64` | signed integers        |
| `u8`, `u16`, `u32`, `u64` | unsigned integers      |
| `f32`, `f64`              | floating-point numbers |

Integer arithmetic stops the program on an overflow while you develop, in a debug build, the default, and wraps around in a production build, the same way in every backend, as in Rust ([types](../reference/types.md)); `wrappingAdd`, `wrappingSub` and `wrappingMul` wrap on purpose. Division by zero stops the program (a _trap_), it never returns a made-up value.

Literals take the type their context asks for, `i32` and `f64` when nothing does. They can be written in hexadecimal, binary, with separators and exponents:

```peras build
export fn literals(): i64 {
  const mask: i64 = 0xFF_FF;
  const flags: i64 = 0b1010;
  const million: i64 = 1_000_000;
  return mask + flags + million;
}
```

```peras build
export fn tiny(): f64 {
  return 1.5e-3;
}
```

> Today, `i32`, `i64`, `f32` and `f64` compile to WebAssembly; the 8-, 16-bit and unsigned types are checked, not compiled yet.

## No implicit conversion

Mixing two number types is an error: you convert explicitly, so a loss of precision is always visible.

```peras error E0210
export fn mean(total: i32, count: i64): i64 {
  return total / count;
}
```

```peras build
export fn mean(total: i32, count: i64): i64 {
  return total.toI64() / count;
}
```

Converting a float to an integer rounds toward zero and saturates: `(1e20).toI32()` is the largest `i32`, never an undefined value.

## Booleans

`bool` is `true` or `false`, and a condition must be a `bool`: Peras has no truthiness.

```peras error E0217
export fn sign(n: i32): i32 {
  if (n) {
    return 1;
  }
  return 0;
}
```

```peras build
export fn sign(n: i32): i32 {
  if (n != 0) {
    return 1;
  }
  return 0;
}
```

`==` and `!=` compare strictly, like `===` and `!==` in JavaScript; Peras has no `===` ([E0006](../errors/E0006.md)).

## Comments

```peras build
// A line comment.
/* A block comment,
   on several lines. */
export fn one(): i32 {
  return 1; // after code, too
}
```

Next: [functions](functions.md).
