# Changelog

_Version française : [CHANGELOG.fr.md](CHANGELOG.fr.md)_

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/). Before 1.0, any release may break the language.

## Unreleased

### Language and compiler

- **Vector loops**: a counted loop whose turns are independent (straight-line body, every element at index `i`, one type among `i32`, `f32` and `f64`, operations exact in every lane, integer reductions `+ ^ | &`) gets a copy running 4 lanes at once (2 for `f64`) with WebAssembly SIMD, before the scalar loop, which always runs the last turn. Results are bit for bit those of the scalar loop: tests compare both on every length from 0 to 19, and break on purpose when the vector code is wrong. The grayscale kernel goes from about 10 to 5 ms, 2.2 times JavaScript's speed. A module with such a loop needs WebAssembly SIMD (Safari 16.4 and later).
- **Bounds checks proven away**: in `for (let i = 0; i < bound; i++)` whose body grows no array and calls nothing, `xs[i]` is no longer checked when `bound` is the length of `xs`. Another array is checked once before the loop, which runs as written when the check fails, so it traps at the same place after the same effects. Nested loops with the same bound inherit the proof. The particles kernel goes from 24 to 21 ms. Reading lengths and addresses before the loop was measured too, and dropped: it was slower.
- **A browser benchmark** (`bench/browser`, `node scripts/bench-browser.mjs`): eight kernels in Peras and in JavaScript, checked equal then timed in Chromium, with download and start time. Peras is 1.2 to 1.3 times faster on numbers in a `PerasArray`, as fast on floating-point arithmetic alone, 1.5 to 1.8 times faster on structures, about 20 times on 64-bit integers; slower on text (3.4 times), on very short calls (1.4 times) and against the native sort of typed arrays (2.3 times). CI checks that both sides agree.
- **Fixed**: an `inout` array passed as a typed array (`Float64Array`…) threw after the call and left its buffer alive. Its values are now copied back with `set()`; when the function changed its length, a `RangeError` is thrown after every buffer is released. The TypeScript declarations accept the typed array of the same numbers (`number[] | Float64Array | PerasArray<"f64">`). The loader's first comment no longer calls every allocator the debug one.
- **Numbers as text**: `toString()` on integers and booleans writes what JavaScript's `String()` writes; `parseI32()` and `parseI64()` on strings give `Some(n)` only for a sign or none, then ASCII digits, in the type's range, and `None` otherwise, never a wrapped value. Four new functions of the runtime, `fmt_int`, `fmt_bool`, `str_is_int` and `str_parse_int`, the first taking an `i64` and the last giving one.
- **Numbers**: `round`, `sin`, `cos`, `tan`, `exp`, `log`, `pow`, `atan2` and `hypot` on floats. Those WebAssembly has no instruction for are imported from JavaScript's `Math`, only by the modules that use them, so they give exactly what JavaScript gives; an `f32` goes through `f64`.
- **Arrays**: `slice`, `concat`, `reverse`, `sort` (numbers, heapsort in place without allocating), `includes` and `indexOf` (numbers, booleans and strings), column by column for arrays of structures.
- **Arrays without a copy**: `__perasHeap.array(kind, init)` makes a `PerasArray`, an array of numbers in the module's memory. Exported functions take it with no copy: lent, changed in place by `inout`, or given to `sink`. `view()` gives a typed array over its elements, valid until the next call into the module. On a million `f32`, reading goes from 5.2 to 1.5 ms and changing in place from 11.5 to 0.8 ms. TypeScript sees `number[] | Float32Array | PerasArray<"f32">`: a typed array is copied, as an array is.
- **Text**: `indexOf`, `includes`, `startsWith`, `endsWith`, `slice`, `trim`, `toUpperCase`, `toLowerCase`, `repeat`, `replace`, `replaceAll` and `split` on strings, and `join` on arrays of strings. Indices count bytes of UTF-8; one inside a character moves back to its start, so a slice is always valid text. They are functions of the runtime, written once in WebAssembly text for both allocators, called through a new IR operation, `buf.runtime`; the interpreter has the same functions in Rust, and the tests check that both give the same results.
- **Arrow functions for array methods**: `map`, `filter`, `reduce`, `find`, `findIndex`, `some` and `every` take `(x) => …`, written in place and compiled as the body of a loop, with no function value at run time; `some`, `every`, `find` and `findIndex` stop at the first element that decides. Parameters lend the elements, as `for…of` does; a value the function moves is seen moved at the next turn (`E0204`). An arrow function anywhere else is refused for now (`E0238`). `reduce` keeps numbers and structures of them.
- `min(b)`, `max(b)` and `clamp(lo, hi)` on every number.
- **A production allocator, TLSF** ([decision 0002](docs/en/decisions/0002-tlsf-allocator.md)): every allocation and release in bounded time, size classes found with `i32.clz` and `i32.ctz`, freed blocks merged at once with their neighbours, and the end of the heap given back. `vite build` links it, the dev server keeps the debug allocator, and `perasc build --allocator tlsf` chooses it. It is 2.7 times faster than the debug allocator on a program that allocates 6 million blocks. Both allocators share the buffer functions, written once.
- **Methods and interfaces.** `impl Point { fn norm(self): f64 { … } }` gives a structure or an enumeration of the module its methods (`E0234` otherwise).
  - `self`, `inout self` or `sink self` comes first; `p.shift(1.0)` needs no `&`, and `p` must be a `let`. A method without `self` is called on its type, `Point.origin()`, and any method can be called that way with its `self` first. `Self` names the type; `impl<T> Stack<T>` gives a generic type its methods.
  - `interface Shape { fn area(self): f64; }` and `impl Shape for Circle { … }`: an implementation provides exactly the interface's methods (`E0235`), with exactly their signatures (`E0236`).
  - An interface is a constraint, `fn total<T: Shape>(…)`: each instance of the function calls the method of its type, resolved when compiling, with no table at run time. Used as a type, it is refused (`E0237`).
- **Tuples**: `(i32, String)`, written `(1, "a")`, read with `.0`, `.1`, nested `t.1.0`. A tuple is a generic structure, kept as its fields; JavaScript sees an array, typed `[number, string]`.
- **Type aliases**: `type Meters = f64;` and `type Grid<T> = Array<Array<T>>;`, interchangeable with what they name; one that contains itself is refused (`E0232`).
- **Module constants compile**: `const AREA: i32 = SIDE * SIDE;` is written where it is used. Its value must be known when compiling (`E0233`). Module `let` stays checked only.
- A structure that contains itself through an array, `struct Tree { children: Array<Tree> }`, no longer crashes the compiler with a stack overflow: it is reported as not compiled yet (`E0900`).
- **Generics**, on functions, structures and enumerations: `fn larger<T: Ord>(a: T, b: T): T`, `struct Pair<A, B>`, `enum Tree<T>`.
  - Six built-in constraints: `Eq`, `Ord`, `Numeric`, `Integer`, `Float` and `Copy`. A generic body is checked once, for every type its parameters may stand for: an operation its constraints do not allow is refused, with a fix that adds the constraint (`E0228`); a use with a type that does not meet one is refused (`E0227`), as is an unknown constraint (`E0226`).
  - Type arguments are inferred from the arguments and from the type expected, or written at the call, `larger<i64>(a, b)`: since chained comparisons are refused, this is never a comparison. When nothing determines them, `E0229`.
  - A value of a type parameter moves, unless the parameter is `Copy`; `Ord` and `Numeric` imply it.
  - Compiled by monomorphization: one copy per set of type arguments the module uses, named after them (`larger__i64`), with no cost at run time. A generic function never called is checked, not compiled. Exported functions cannot be generic (`E0230`), and generic code that would need copies for ever larger types is refused (`E0231`).
  - Generic structures and enumerations reach JavaScript as interfaces and unions named after their arguments, `Pair_i32_String`.
- `unwrapOr` with a fallback read from a variable holding a string or an array no longer produces code the verifier rejects: the fallback is copied.
- **Structures** compile, as values: each one is replaced by its scalar fields, nested structures flattened, so it never touches the heap.
  - A structure parameter becomes one parameter per field.
  - A function returning a structure returns one WebAssembly result per field, and a tuple in the Rust backend.
  - Fields are read and written, compound assignments included; a structure is copied by assignment; `p = Point(p.y, p.x)` swaps.
  - Calls returning a structure run first, into temporaries, without changing the order of evaluation: on the right of `&&` and `||` only when it would run, and in a loop condition at every turn.
  - Exported functions take and return JavaScript objects, declared as TypeScript interfaces.
- **`Result<T, E>`**: `Ok(value)` or `Err(error)`, `isOk`, `isErr`, `unwrapOr`, and `?`, which returns the error early; the function must return a result with the same type of error, or an option for `?` on an option (`E0225`). The error moves into the return, buffers included.
- Exhaustiveness and unreachable arms are now checked by the algorithm of Maranget, so nested patterns count: `Err(Empty)` and `Err(NotADigit(_))` together cover `Err`. A type of the module may take the name of a built-in one, such as `Result`.
- **Enumerations with data, and `match`.** `enum Shape { Circle(f64), Rect(f64, f64), Empty }`; values are written `Shape.Circle(1.0)`.
  - `match` takes a value apart with patterns: `_`, names, numbers and booleans, variants with their data, nested; arms are values or blocks, and `return`, `break`, `continue` work inside.
  - Every `match` must cover every value (`E0222`); an unknown variant (`E0223`) and an arm no value can reach (`E0224`) are errors too. A `match` whose arms all return ends the function.
  - Options match too (`Some(x)`, `None`), and an option or an enumeration can now hold strings and arrays: the data of the variants not taken are empty buffers, so the value always owns the same ones. `pop()` on an array of strings now works.
  - JavaScript sees `{ kind: "Circle", values: [1] }`, declared to TypeScript as a union.
- **Arrays of strings and of arrays** compile, and structures holding them in arrays: a buffer can own buffers, which the runtime releases and copies with it, at any depth. JavaScript passes and receives them as nested arrays and objects.
- E0204 tells the value's history: where it received its value, where it moved, where it is used again.
- **Arrays of structures and options** compile, as columns: one buffer per scalar of an element, so `points[i].x` and a loop over one field touch one buffer. Indexing, `points[i].x = v`, `push`, `pop`, `copy`, `==` and `for…of` work column by column; JavaScript sees arrays of objects.
- **`inout`** works on every type: the function gives its numbers back as extra results, written back into the caller's variables; arrays and strings change in place. From JavaScript, a structure passed `inout` is an object whose fields are updated. A variable passed `inout` cannot be read elsewhere in the same statement (`E0207`), and different fields of one structure no longer count as aliases.
- `?` now releases the arrays and strings a function holds before it returns `None`; they leaked before.
- **Strings** compile, in UTF-8: literals, `+`, `+=`, `==`, `!=`, `length` in bytes and `copy()`. A literal is stored once, as a passive data segment, and copied into a new buffer with `memory.init`; `+=` appends in place. JavaScript passes and receives strings, encoded and decoded as UTF-8. `==` also compares arrays, element by element.
- **Arrays** compile, of numbers and booleans, on the heap: literals, `length`, indexing and assignment through an index, `push`, `pop`, `copy`, `for…of`.
  - Every index is checked; one out of bounds stops the program with a `RangeError`, in English or French, before memory is touched.
  - An array is a 12-byte header, which keeps its address, and its elements; `push` doubles the room when it runs out. The debug allocator holds both blocks, so every memory bug stays caught.
  - JavaScript passes and receives copies, as `number[]`, `bigint[]` or `boolean[]`; an `inout` parameter is copied back.
- **Ownership** is checked: a value on the heap has one owner, moves on assignment, `return` and `sink`, and is borrowed otherwise.
  - Four new errors: a value used after it moved ([E0204](docs/en/errors/E0204.md)), `&` missing or extra on an `inout` argument ([E0206](docs/en/errors/E0206.md)), two arguments reaching one variable that one of them changes, or an array changed while `for…of` goes through it ([E0207](docs/en/errors/E0207.md)), moving what is only borrowed ([E0209](docs/en/errors/E0209.md)). Each fix adds `.copy()`, `&` or `sink` where it applies.
  - Lowering inserts every release: at the end of a scope, on reassignment, before `return`, `break` and `continue`, and where paths meet after a move on one of them. The release verifier proves each one; the test programs leave no block alive, in the interpreter and as WebAssembly on the debug allocator.
- **Options** compile, also without the heap: an `Option<T>` is a `bool` followed by the scalars of `T`, any compiled `T`, structures included.
  - `Some`, `None`, `isSome`, `isNone`, `unwrapOr`, and `?`, which returns `None` early from a function returning an option.
  - `checkedAdd`, `checkedSub` and `checkedMul` on `i32` and `i64` give `None` on overflow; they are tested against Rust's own on the values where overflow starts and stops.
  - `None` and `[]` take the type their context expects.
  - JavaScript sees the value or `undefined`, declared as `T | undefined`.
- The JavaScript loader prefixes every name it declares with `$`, which a Peras name cannot start with: a function or a parameter named `r`, `url` or `runtime` no longer breaks the module.
- The IR verifies ownership: `owned` and `borrowed` handles, explicit releases, and a verifier proving that every block is released exactly once on every path. The debug allocator is linked into modules that use the heap; a memory bug throws a `PerasMemoryError` naming both sites, in English or French. Nothing in the language allocates yet.

### Benchmark

- `overflows` exercises options and checked arithmetic; both backends agree with the JavaScript reference.
- `cmandel` computes the Mandelbrot set with `struct Complex`. Both backends agree with the JavaScript reference. The direct backend takes 15 ms against 13 ms for plain `f64` code and for Rust; JavaScript, which allocates an object per complex number, takes 49 ms. The optimizer does not touch functions with several results yet.

## 0.0.1 — 2026-11-04

The end of phase 0 of the [kickoff plan](docs/en/kickoff-plan.md): a `.peras` function compiles to WebAssembly and runs in the browser through Vite, and every error shows an excerpt, a cause and a fix, in English and French.

### Language and compiler

- Lexer, parser, name and type checks for functions, `let` and `const`, `struct`, arrays, `if`, `while`, `for` and `for…of`. Every error is reported in one pass, with a stable code, a cause, a fix as a diff, and a `perasc explain` page.
- 46 error codes, each with its page in English and French; the example of every page is checked to produce its code and its fix to remove it.
- `perasc check`, `perasc build` and `perasc explain`, with `--lang en|fr`, `--json` and `--color`.
- Code generation for functions on numbers and booleans:
  - a direct WebAssembly backend, the default, which needs no other tool;
  - a Rust backend, used in CI as a check of the direct one ([decision 0001](docs/en/decisions/0001-webassembly-backend.md)).
- IR optimizations, each measured against Rust and checked by an IR verifier and a differential test:
  - accumulator recursion elimination;
  - recursion unrolling;
  - exit test rotation;
  - branchless integer `abs`;
  - doubling as an addition;
  - loop-invariant code motion.

  On the benchmark kernels, the direct backend is level with the Rust backend.

### Tooling

- `vite-plugin-peras` imports a `.peras` file as a module. It shows errors in Vite's overlay, fails `vite build` on an error, and writes TypeScript declarations next to the source.
- `examples/fib` runs `fib` in the browser.

### Quality gates

Every check blocks the merge:

- formatting;
- lints with no warning;
- unit tests, UI tests and end-to-end tests in headless Chromium;
- `cargo deny` and `pnpm audit` with no known CVE;
- the two backends agreeing;
- translations complete;
- nightly fuzzing of the lexer, the parser and the whole pipeline.

### Known limits

- Strings, arrays, structures, methods and `Option` are checked but not compiled yet: code generation reports them with `E0900`.
- Ownership, `.apeiron` modules and the standard library come in the next phases.
