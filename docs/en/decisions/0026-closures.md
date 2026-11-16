# 0026: Closures in owned modules

_French translation: [../../fr/decisions/0026-closures.md](../../fr/decisions/0026-closures.md)._

_Status: proposed on 2026-11-16, with its criteria registered before any code; it follows [decision 0025](0025-moved-fields-and-function-values.md), whose refusal `closure` it lifts._

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

- **The runtime refuses to release address 0** (`INVALID_RELEASE`), so an owned value always holds a real block. An empty `Option<String>` holds an empty buffer, made for it.
- **The IR has no reinterpretation of bits** between floats and integers. WebAssembly has one: `i64.reinterpret_f64` and the others.
- **The Rust backend compiles no buffer**: an operation on one is `unreachable!()` there.

## Decision

1. **A function value carries an environment.** It is two scalars: its position in the table, an `i32`, and its environment, an owned buffer of `i64`, one slot per captured value.
   - A function of the module, or an arrow function that captures nothing, has an empty environment, made where the value is made.
   - In an owned module, a value of a function type therefore moves as a string does:
     - `const g = f;` moves `f`;
     - an argument is lent;
     - `f.copy()` copies the environment;
     - leaving its scope releases it.
2. **What a closure captures, in an owned module, cannot change while it lives.**
   - It captures a `const`, or a parameter that is not `inout`.
   - A `let` it captures is refused, with a new code. A copy made when the closure is made is then what JavaScript's reference would read, so both modes give the same.
3. **What it captures is copied into its environment when it is made.**
   - Numbers and booleans are stored as 64 bits: an integer extended by its type, a float by its bits. For that, the IR gains the reinterpretations of WebAssembly.
   - A capture of a string, an array, a map, a set or a structure holding one is refused for now (`E0900`, `closure-heap`). It is the next step: moving it into the environment, which would then own buffers.
   - A function value captured is refused too, for the same reason: it holds a buffer.
4. **Every function in the table takes its environment first**, lent.
   - A closure reads each capture from it, by its slot.
   - A function of the module used as a value gets an adapter in the table, which takes the environment and calls the function. Its direct calls are unchanged.
   - `xs.map(twice)` stays a direct call: the value is never made.
5. **A managed module is unchanged.** Its closures are JavaScript's.
6. **The Rust backend** refuses a function value with `E0900`, since it compiles no buffer, where it compiled the `i32` of decision 0025. This is written as a regression of that backend, not hidden.

## Not chosen

- **Keeping the `i32` and putting environments elsewhere**: in a table of the module, or never released. That is either a second owner to keep in step, or a leak.
- **Capturing by reference, as JavaScript does**: a boxed variable, shared by the function and the closure. Sharing needs reference counting, which is `Rc<T>`, finding F310, still to design.
- **A static empty environment for functions that capture nothing**: an owned value must be a block the runtime can release. A borrowed one would make two kinds of function values, which one type cannot tell apart.
- **Copying captured strings implicitly**: Peras makes copies explicit (`copy()`), and a closure that copies in secret would be the only place it did not.

## Criteria, registered before any code

The step lands only with all of these, in addition to [decision 0021's criteria](0021-phases-4-and-5.md#criteria-of-each-step):

1. **UI tests of the checker:**
   - refused:
     - a `let` captured, with the new code;
     - a string, an array, a structure holding one, and a function value captured (`E0900`, `closure-heap`);
   - accepted: a `const` and a parameter captured;
   - in an owned module, a function value moves:
     - `const g = f;` then `f(1)` is `E0204`;
     - `f.copy()` keeps both.
2. **End to end, on both allocators,** against the same source built as a managed module, which is JavaScript. The calls:
   - `makeAdder(k)` returning a closure;
   - an array of closures over a loop's `const`, called after the loop;
   - a closure in a structure and in an option;
   - a closure given to `apply`;
   - `f.copy()`;
   - closures over `i8`, `u32`, `i64`, `u64`, `f32`, `f64` and `bool`, the edges of each.

   The results are the same, and nothing is left alive.

3. **The differential test** gains a closure made in a loop over the turn's values and called through an array of functions, and passes 2,000 seeds. Reading the wrong slot of the environment, on purpose, must fail it.
4. **The release verifier** proves every environment released once: made and dropped, moved into an array, returned, replaced. An environment not released, on purpose, is refused.
5. **The reinterpretations** are tested against Rust's `to_bits` and `from_bits` on every edge of `f32` and `f64`, NaN and −0 included, in the interpreter and in WebAssembly.
6. **Measures, reported and not conditions:**
   - a call through a closure against a direct call, in one process (finding M017);
   - the dashboard, which makes no function value, unchanged by `ab-data.mjs`;
   - the size of a module with a closure.

   **One number is a condition:** the test of decision 0025 that calls a named function through a value 20,000,000 times must stay within 1.5 times its time of decision 0025, measured in one process. Every function value now makes an environment.

7. **Docs,** in English and in French:
   - the guide's page on functions;
   - the status, with closures compiled for numbers and booleans;
   - `E0900` and the new code;
   - the CLI reference, for the Rust backend (`--backend rust`);
   - finding F308's follow-up.

## Record

_Each part is recorded here as it lands._
