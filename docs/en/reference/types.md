# Types

## Numbers

| Type  | Size    | Range                     | Status                                        |
| ----- | ------- | ------------------------- | --------------------------------------------- |
| `i8`  | 8 bits  | −128 to 127               | <span class="status checked">checked</span>   |
| `i16` | 16 bits | −32,768 to 32,767         | <span class="status checked">checked</span>   |
| `i32` | 32 bits | about ±2.1 × 10⁹          | <span class="status compiled">compiled</span> |
| `i64` | 64 bits | about ±9.2 × 10¹⁸         | <span class="status compiled">compiled</span> |
| `u8`  | 8 bits  | 0 to 255                  | <span class="status checked">checked</span>   |
| `u16` | 16 bits | 0 to 65,535               | <span class="status checked">checked</span>   |
| `u32` | 32 bits | 0 to about 4.3 × 10⁹      | <span class="status checked">checked</span>   |
| `u64` | 64 bits | 0 to about 1.8 × 10¹⁹     | <span class="status checked">checked</span>   |
| `f32` | 32 bits | IEEE 754 single precision | <span class="status compiled">compiled</span> |
| `f64` | 64 bits | IEEE 754 double precision | <span class="status compiled">compiled</span> |

Rules, the same in every backend:

- integer `+`, `-`, `*` wrap around on overflow; `checkedAdd` and the others return `None` instead, `wrappingAdd` and the others say it explicitly;
- integer division and remainder by zero stop the program, as does `MIN / -1`;
- float to integer conversions round toward zero and saturate; NaN becomes 0;
- no conversion is implicit: [E0210](../errors/E0210.md), [E0211](../errors/E0211.md).

### Literals

| Form                       | Example                   | Type                                         |
| -------------------------- | ------------------------- | -------------------------------------------- |
| decimal                    | `42`, `1_000_000`         | the context's integer type, `i32` by default |
| hexadecimal, octal, binary | `0xFF`, `0o755`, `0b1010` | same                                         |
| float                      | `3.14`, `1.5e-3`, `2e10`  | the context's float type, `f64` by default   |

A literal that does not fit its type is an error ([E0212](../errors/E0212.md)).

## `bool`

`true` or `false`. Conditions must be `bool` ([E0217](../errors/E0217.md)). <span class="status compiled">compiled</span>

## `void`

The result of a function that returns nothing. <span class="status compiled">compiled</span>

## Structures

`struct Name { field: Type; … }`: a value made of named fields, built with `Name(…)` and copied by assignment. See [structures](../guide/structures.md). <span class="status compiled">compiled</span>

## `Array<T>`

A growable list of values of type `T`, written `[a, b]`, compiled when `T` is a number, `bool`, or a structure or option of them. See [collections](../guide/collections.md#arrayt). <span class="status compiled">compiled</span>

## `String`

UTF-8 text, written `"…"`; its `length` counts bytes. See [collections](../guide/collections.md#string). <span class="status compiled">compiled</span>

## `Option<T>`

`Some(value)` or `None`: a value that may be absent. Peras has no `null` and no `undefined`; JavaScript sees an empty option as `undefined`. <span class="status compiled">compiled</span>

## Not in the language yet

Enumerations are on their own page: [enumerations and match](../guide/enums.md). Results are with them: `Result<T, E>`, `Ok(value)` or `Err(error)`. <span class="status compiled">compiled</span>

Unions, generics, tuples, `Map`, `Set`, function types: see the [roadmap](roadmap.md).
