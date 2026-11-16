# 0026: Closures in owned modules

_French translation: [../../fr/decisions/0026-closures.md](../../fr/decisions/0026-closures.md)._

_Status: accepted by the owner on 2026-11-16, with its criteria registered before any code; it follows [decision 0025](0025-moved-fields-and-function-values.md), whose refusal `closure` it lifts. A first version, proposed the same day, made the environment an owned value that moves; the owner asked whether its regressions were not too high, and this version removes them._

## Context

What the code does today, read before writing this (finding M020), and what the compiler says.

```peras
fn apply(f: (i32) => i32, x: i32): i32 {
  return f(x);
}

export fn shifted(n: i32, k: i32): i32 {
  return apply((x) => x + k, n); // E0900: arrow functions that capture a variable, used as values
}
```

- **A function value is an `i32`** since decision 0025: its position in the module's table, called by `call_indirect`. Nothing else travels with it, so a function that needs a captured `k` has nowhere to find it.
- **A function value is copied freely**: `const g = f;` keeps `f` usable, and nothing is released.
- **An arrow function given in place** to `map` and the others is lowered in the loop. It reads the enclosing locals directly, and captures nothing.
- **An arrow function used as a value** becomes a function of its own, lowered after the others in a state of its own. A name of the enclosing function is unknown there, which is how the refusal `closure` is found.
- **A managed module captures by reference**, as JavaScript does. Below, `later(5)` gives 105, not 6:

  ```apeiron
  export fn later(n: i32): i32 {
    let k = 1;
    const f = (x: i32): i32 => x + k;
    k = 100;
    return apply(f, n);
  }
  ```

- **The runtime refuses to release address 0** (`INVALID_RELEASE`), so an owned value always holds a real block.
- **A buffer's header** keeps, at its offset 12, the size of an element and its flags (`DEEP`, `INLINE`, `MAP`, `STRKEY`). Its capacity is at offset 8.
- **The IR has no reinterpretation of bits** between floats and integers. WebAssembly has one: `i64.reinterpret_f64` and the others.
- **The Rust backend compiles no buffer**: an operation on one is `unreachable!()` there. It compiles the `i32` of decision 0025.

## Decision

1. **A function value carries an environment.** It is two scalars: its position in the table, an `i32`, and its environment, an `i32` handle.
2. **An environment is frozen and shared.** It is a block of `i64` slots, one per captured value, written when the closure is made and never after.
   - Its header counts who holds it. A copy of the value adds one; leaving a scope takes one away, and the last one frees the block.
   - A function value therefore stays a value that copies freely, as in decision 0025: `const g = f;` keeps `f`, and nothing moves.
   - Nothing it holds can hold it back, since it holds only numbers: no cycle can keep it alive.
3. **A value that captures nothing has a static empty environment**, written once at the module's start below the heap: a named function given as a value, or an arrow function that captures nothing. Adding to or taking from its count does nothing, and it is never freed, so such a value costs what it costs in decision 0025: no allocation.
4. **What a closure captures, in an owned module, cannot change while it lives.**
   - It captures a `const`, or a parameter that is not `inout`.
   - A `let` it captures is refused, with a new code. A copy made when the closure is made is then what JavaScript's reference would read, so both modes give the same.
5. **What it captures is copied into its environment when it is made.**
   - Numbers and booleans are stored as 64 bits: an integer extended by its type, a float by its bits. For that, the IR gains the reinterpretations of WebAssembly.
   - A capture of a string, an array, a map, a set, a structure holding one, or a function value is refused for now (`E0900`, `closure-heap`). Holding it would make the environment own a buffer: the next step.
6. **Every function in the table takes its environment last**, lent, after its arguments: the arguments are evaluated before the value called, so that the call reads its function's position from the environment without a second local per nested call.
   - A closure reads each capture from it, by its slot.
   - A function of the module used as a value gets an adapter in the table, which takes the environment and calls the function. Its direct calls are unchanged.
   - `xs.map(twice)` stays a direct call: the value is never made.
7. **A managed module is unchanged.** Its closures are JavaScript's.
8. **The Rust backend loses nothing.** A value that captures nothing compiles there as in decision 0025: its environment is a constant, adding to and taking from the count do nothing. A closure that captures is refused there, as every other value on the heap is, since that backend compiles no buffer.

## Not chosen

- **An environment owned by one value, which moves**, the first version of this decision: `const g = f;` would have moved `f`, a new allocation would have been made for every named function given as a value, and the Rust backend would have lost every function value. Three regressions for one gain: no count to keep.
- **Keeping the `i32` and putting environments elsewhere**: in a table of the module, or never released. That is either a second owner to keep in step, or a leak.
- **Capturing by reference, as JavaScript does**: a boxed variable, shared by the function and the closure, which may change it. That is `Rc<RefCell<T>>`, finding F310, still to design; a frozen environment needs only the count.
- **Copying captured strings implicitly**: Peras makes copies explicit (`copy()`), and a closure that copies in secret would be the only place it did not.

## Criteria, registered before any code

The step lands only with all of these, in addition to [decision 0021's criteria](0021-phases-4-and-5.md#criteria-of-each-step):

1. **UI tests of the checker:**
   - refused:
     - a `let` captured, with the new code;
     - a string, an array, a structure holding one, and a function value captured (`E0900`, `closure-heap`);
   - accepted: a `const` and a parameter captured; `const g = f;` then `f(1)` and `g(1)`.
2. **End to end, on both allocators,** against the same source built as a managed module, which is JavaScript. The calls:
   - `makeAdder(k)` returning a closure;
   - an array of closures over a loop's `const`, called after the loop;
   - a closure in a structure and in an option;
   - a closure given to `apply`, and copied before being called;
   - closures over `i8`, `u32`, `i64`, `u64`, `f32`, `f64` and `bool`, the edges of each.

   The results are the same, and nothing is left alive.

3. **The count holds.** A closure copied 100 times into an array, the array then released, leaves nothing alive; a copy that does not add to the count, made on purpose in the lowering, is caught by a test (a block freed twice, or released while read).
4. **The differential test** gains a closure made in a loop over the turn's values and called through an array of functions, and passes 2,000 seeds. Reading the wrong slot of the environment, on purpose, must fail it.
5. **The release verifier** proves every environment released on every path: made and dropped, copied into an array, returned, replaced. An environment not released, on purpose, is refused.
6. **The reinterpretations** are tested against Rust's `to_bits` and `from_bits` on every edge of `f32` and `f64`, NaN and −0 included, in the interpreter and in WebAssembly.
7. **No regression of the Rust backend:** a module whose function values capture nothing compiles with `--backend rust` and gives what the WebAssembly module gives, held by a test of the generated text.
8. **Measures, reported and not conditions:**
   - a call through a closure against a direct call, in one process (finding M017);
   - the call through a named function given as a value, against its time in decision 0025, in one process;
   - the dashboard, which makes no function value, unchanged by `ab-data.mjs`;
   - the size of a module with a closure.
9. **Docs,** in English and in French:
   - the guide's page on functions;
   - the status, with closures compiled for numbers and booleans;
   - `E0900` and the new code;
   - the CLI reference, for the Rust backend (`--backend rust`);
   - finding F308's follow-up.

## Record

_Each part is recorded here as it lands._

- **2026-11-16, the step, in one batch.**
  - The IR gains `ExprKind::Env` (`Static`, `New`, `Zero`, `Load`) and `UnOp::Bits`; a function type lowers to one `owned` leaf, as a string does. Every function in the table takes its environment last: the arguments are evaluated before the value called, so the call reads its function's position from it with one scratch local.
  - The runtime marks an environment `SHARED`: `buf_clone` adds one to its count, `buf_release` takes one and frees the block at zero; a static environment, written below the heap by a data segment, has a count of 0, which neither changes. The empty environment of a `None` is at the fixed address `ZERO_ENV`, and literals start after it.
  - A module whose environments are all static (`Module::static_envs_only`) needs no heap: an environment is then its function's position, a copy is the same position, and a release does nothing. This is how the Rust backend keeps compiling decision 0025's values.
  - The checker records what each arrow function captures, and refuses, in an owned module, a `let` or an `inout` parameter captured by one used as a value (`E0251`).
  - Criteria:
    1. `E0251-closure-capture.peras` and `.apeiron`, `E0900-closure.peras` (`closure-heap`: a string, an array, a structure holding one, a function value), and `closures.peras`, whose IR is kept.
    2. `closures.test.ts`, on both allocators, against the same source built as a managed module: `makeAdder`, closures over a loop's `const`, a closure in a structure and in an option, given to `apply` and copied, closures over the edges of `i8`, `u32`, `i64`, `u64`, `f32`, `f64` and `bool`. The results are the same, and nothing is left alive.
    3. `an_environment_copied_is_freed_once`: 100 copies in an array, then released, leave nothing alive in the interpreter and in WebAssembly. A copy that does not count, on purpose in the runtime's `buf_clone`, makes the debug allocator stop the call; in the interpreter, a `DoubleRelease`.
    4. The differential test gains closures made in a loop and called through an array; it passes 2,000 seeds. Reading slot 0 for every slot, on purpose in the WebAssembly generator, fails both differential tests (seed 28 among others). The same mutation in the lowering passed: finding M026.
    5. The release verifier refuses an environment made and never released, on purpose in the lowering: `%3 still holds a block when the function returns: it leaks`.
    6. `reinterpretations_match_rusts_bits` and `captured_numbers_come_back_bit_for_bit`: every edge of `f32` and `f64`, NaN with a payload, a signalling NaN and −0 included, in the interpreter and in WebAssembly.
    7. `static_environments_are_positions` holds the Rust written; and a module of values that capture nothing, built with `--backend rust` and with the direct backend, gave the same 16 results in Node. `bench-backends.mjs --check` passes.
    8. Measures, after the commit of the step (`3a750e8`), on an Intel Xeon at 2.10 GHz, 4 cores, TLSF, 21 rounds of 20,000,000 calls in one process, the order alternating (`calls.mjs`, kept outside the repository):
       - through a closure, 77.9 ms against 81.8 ms directly, a ratio of 0.941 in the median, from 0.874 to 1.128;
       - a named function through a value, 75.7 ms against 75.5 ms with decision 0025's compiler (`1daaf30`), a ratio of 1.007, from 0.733 to 1.083: no cost found for the static environment;
       - the module with a named function as a value is 224 bytes against 169 in decision 0025: 55 bytes for the adapter and the environment's parameter. With a closure, 3,444 bytes: a counted environment links the allocator, as any heap value does;
       - the dashboard, which makes no function value, `node scripts/ab-data.mjs 1daaf30`, 1,000,000 rows, 20 rounds: 344.8 ms before, 353.1 ms after, a ratio before / after of 1.011 in the median, from 0.775 to 1.314, the same summary.

       Reported, not conditions.

    9. The guide's page on functions, the status, `E0900`, the new `E0251` and the index of errors (which lacked `E0250` too), the CLI reference, findings F308, F011 and M026, the roadmap and the changelog, in English and in French.
- **Departures from the text above.**
  - The environment is the last parameter, not the first.
  - A closure that captures is refused by the Rust backend's driver as every heap value is, not with `E0900`.
  - Criterion 4's mutation is made in the generator, not the lowering (finding M026).
  - Bug F011 was found and fixed on the way: two arrow functions used as values had the same name.
