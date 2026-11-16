# 0024: Every integer type compiled in owned modules

_French translation: [../../fr/decisions/0024-integer-types.md](../../fr/decisions/0024-integer-types.md)._

_Status: proposed on 2026-11-16, with its criteria registered before any code; step 4 of [decision 0021](0021-phases-4-and-5.md), finding F302._

## Context

Peras has eight integer types: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32` and `u64`. The checker knows all eight, and a managed module compiles all eight to JavaScript. An owned module compiles `i32` and `i64` only; the other six are refused with `E0900` (`integer-type`).

What the code does today (read before writing this, after finding M020):

- **The checker** never converts between them implicitly. `-` on an unsigned type is refused (`E0221`). A shift takes a count of the same type as the value. `toU8()` and the other conversions are methods.
- **The IR** has two integer types, `i32` and `i64`. Its operators are signed: `/` and `%` truncate toward zero with the sign of the dividend, `>>` is arithmetic, and the comparisons are signed. A conversion is `Convert(x)`, typed by its result, with no word on the signedness of its operand.
- **Arrays** store their elements by `Elem`. `Elem::U8`, one byte read as an `i32` from 0 to 255, already holds the bytes of strings.
- **The loader** checks no scalar coming from JavaScript: a number passed as an `i32` is reduced modulo 2³², as WebAssembly itself does. A `bool` is an `i32` in WebAssembly and a boolean in JavaScript: the loader already tells apart two types that share one representation.
- **A bug found while reading.** `int_const` turns a `u64` literal above `i64::MAX` into `i64::MIN`, not into its bits. Nothing reaches it today, since `u64` is refused.
- **A managed module** gives `i64` and `u64` as `BigInt`s and the others as numbers, and checks each result against the range of its type. It is the reference for what each operation gives.

## Decision

1. **No new type in the IR: the operations carry the signedness.**
   - `i8`, `i16`, `u8`, `u16` and `u32` are kept in an `i32`, and `u64` in an `i64`.
   - A value is always in its canonical form: an `i8` or an `i16` sign-extended, a `u8` or a `u16` zero-extended. So `==`, `!=` and the bitwise operators need nothing new.
   - The IR gains unsigned division, remainder, right shift and comparisons, and conversions that say the signedness of their operand. WebAssembly has an instruction for each.
   - New IR types would have been the other way. Every pass that compares a type with `i32` or `i64` would then have to learn six more, and a pass that forgot one would mistreat it. With the operations carrying the signedness, a pass that knows nothing of them sees `i32` and `i64` operations it already handles.
2. **The rule of overflow is the one `i32` and `i64` already follow** ([decision 0011](0011-what-to-build-next.md)).
   - `+`, `-` and `*` stop the program in a debug build when the result leaves the range of its type, and wrap to its width in a production build.
   - `wrappingAdd` and the others always wrap, and `checkedAdd` and the others give `None`.
   - Division by zero stops the program in both builds.
3. **Arrays store each element at its own width.**
   - `Array<u8>` uses the existing `Elem::U8`. `Array<i8>`, `Array<u16>` and `Array<i16>` get elements of one and two bytes, read with the right extension.
   - `Array<u32>` and `Array<u64>` share the storage of `i32` and `i64`, and are read as unsigned at the boundary.
   - A `PerasArray` gains the kinds `i8`, `i16`, `u16`, `u32` and `u64`: `Int8Array`, `Int16Array`, `Uint16Array`, `Uint32Array` and `BigUint64Array`.
4. **At the boundary with JavaScript.**
   - Every type but `i64` and `u64` is a number, and those two are `BigInt`s.
   - A value given by JavaScript is wrapped to its type, as a typed array of that type stores it: 300 given as a `u8` is 44, as `new Uint8Array([300])` holds. This is how an `i32` is already taken.
   - A `u32` or a `u64` given back is never negative.
5. **Maps and sets** keyed by these types, and module variables of them, follow the same rules. A key is widened to 64 bits by the extension of its type.
6. **The literal bug is fixed** with the rest: a `u64` literal keeps its bits.

## Not chosen

- **Checking the values JavaScript gives**, a `RangeError` for 300 given as a `u8`. It would make `u8` stricter than `i32` is today. If it is wanted, it should come for every integer type at once, in its own decision.
- **New types in the IR,** for the reason given in point 1.

## Criteria, registered before any code

The step lands only with all of these, in addition to [decision 0021's criteria](0021-phases-4-and-5.md#criteria-of-each-step):

1. **Against an oracle that is not the compiler.** For each of the six types, a test in Rust computes random operations with Rust's own arithmetic on that type and with the compiled module, run by `wasmi`. It covers 10,000 cases per type in a release build, and fewer in debug, timed under a minute (finding M022).
   - The operations are `+`, `-` and `*` in their wrapping form, `/`, `%`, `<<`, `>>`, the bitwise operators, the comparisons, and the conversions to and from every other numeric type.
   - The operands include the edges: 0, 1, the minimum, the maximum and −1.
   - The results must be equal bit for bit.
2. **Overflow.** For each type, an addition and a multiplication past its maximum stop the program in a debug build and wrap in a production build. The same for a subtraction below its minimum.
3. **End to end, on both allocators.**
   - Every type as a parameter, a result, a field of a structure and an element of an array, against the same calls in JavaScript.
   - The values given by JavaScript include out-of-range numbers, which are wrapped.
   - A `u32` above 2³¹ and a `u64` above 2⁶³ come back non-negative.
   - Nothing is left alive.
4. **The differential test** gains `u8`, `u16`, `u32`, `u64`, `i8` and `i16` locals in its loops, and runs with 2,000 seeds. Two changes made on purpose must each fail a test: leaving out the narrowing after an `u8` addition, and comparing two `u32` as signed.
5. **Docs:** the types reference, the status and `E0900`, in English and in French.

## Record

_Each part is recorded here as it lands._
