# 0024: Every integer type compiled in owned modules

_French translation: [../../fr/decisions/0024-integer-types.md](../../fr/decisions/0024-integer-types.md)._

_Status: proposed on 2026-11-16, with its criteria registered before any code; accepted by the owner the same day; built, its criteria met (see [Record](#record)); step 4 of [decision 0021](0021-phases-4-and-5.md), finding F302._

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

**Every integer type in owned modules: landed.**

- **What changed.**
  - The IR gains `convert_u`, a conversion that reads or makes an unsigned integer, and the unsigned `/u`, `%u`, `>>u`, `<u`, `<=u`, `>u` and `>=u`; WebAssembly has an instruction for each, the Rust backend casts through `u32` or `u64`, and the interpreter computes them as Rust does. The release verifier checks their types.
  - The lowering, `lower/ints.rs`, keeps each value canonical: after `+`, `-`, `*`, `<<`, `-x` and `~x`, a narrow type is masked or sign-extended back. With overflow checks, a narrow type compares the exact result with its range, `u32` and `u64` by an unsigned comparison or a division; `MIN / -1` of an `i8` or an `i16` stops the program in every build, as it does for `i32`. A conversion from a float to a narrow type saturates to an `i32`, then to the type.
  - Arrays store each element at its width: `Elem` gains `I8`, `I16`, `U16`, `U32` and `U64`, loaded with the right extension. The runtime's `arr_sort` and `arr_sort_index` take unsigned kinds and elements of one or two bytes, widened to sort them, and `buf_equal` elements of two bytes.
  - The loader wraps each value JavaScript gives to its type, in a parameter, a field, an array, a map or a set, gives `u32` and `u64` back non-negative, and a `PerasArray` has the kinds `i8`, `i16`, `u16`, `u32` and `u64`. A key of a map or a set is widened by its type's extension, a `u32` by zeros.
  - `u64` is formatted by `fmt_uint`, new in the runtime; a `u64` literal above `i64::MAX` keeps its bits.
- **Tests, each criterion in turn.**
  1. `crates/perasc-driver/tests/integers.rs`: for each of the six types, a module of 29 functions, the wrapping `+`, `-` and `*`, `/`, `%`, `<<`, `>>`, `&`, `|`, `^`, `min`, `max`, the five comparisons, the conversions to every numeric type and from `f32` and `f64`, against Rust's arithmetic on that type, bit for bit: 10,000 cases per type in release, 400 in debug, under a second either way. The edges are drawn one time in four; the floats include NaN, the infinities and values past the type's range.
  2. `overflow.rs`: for each type, `+` and `*` past the maximum and `-` below the minimum stop a debug build and wrap in production; `MIN / -1` stops both.
  3. `integers.test.ts`, on both allocators: every type as a parameter, a result, a field, an array element, an `inout` array, a map key, a set element and a module variable, against the same calls written in JavaScript, with out-of-range numbers given; `u32` above 2³¹ and `u64` above 2⁶³ come back non-negative; `PerasArray`s of each kind are viewed as their typed arrays; nothing is left alive.
  4. The differential test gains `u8`, `u16`, `u32`, `u64`, `i8` and `i16` locals and an `Array<i16>` sorted short and past 64 elements; it passes 2,000 seeds. Leaving out the narrowing after a `u8` addition fails `overflow.rs` (`u8 add wrapped`); comparing `u32` as signed fails the oracle at its first cases. The differential test's oracle is the IR's interpreter: it sees an optimization that mishandles these operations, not a lowering that does, which the oracle sees.
  5. The types reference, the status, `E0900`, `E0249` and the method reference, in English and in French.
- **Found on the way**, each fixed with a test that fails without it: the vectorizer read an array of bytes as lanes of four (F007); `is_simple` let an unsigned division leave a loop that does not turn (F008); `reverse()` mixed the bytes of elements of two bytes (F009).
- **Departures from the text above.**
  - In an array of structures, a field of these types takes the four bytes of an `i32` (or the eight of an `i64`): only an `Array<T>` of one of them is stored at its width. The loader wraps such a field all the same.
  - `sort()`, `includes`, `indexOf` and the keys of `sortBy` took only `i32`, `i64` and floats, in both modes (F109); they take every integer type now, a change of the checker the text above did not name.
  - The Rust backend writes these operations through `u32` and `u64`, which a unit test checks in the text it writes (`unsigned_operations_go_through_unsigned_types`); a build by `--backend rust` was not run end to end.
