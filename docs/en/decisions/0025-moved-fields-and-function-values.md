# 0025: A field moved out of a structure, and functions as values

_French translation: [../../fr/decisions/0025-moved-fields-and-function-values.md](../../fr/decisions/0025-moved-fields-and-function-values.md)._

_Status: proposed on 2026-11-16, with its criteria registered before any code; accepted by the owner the same day, part B with real function values, both parts to land as one batch; built, its criteria met (see [Record](#record)); the last part of step 4 of [decision 0021](0021-phases-4-and-5.md), findings F214 and F308._

## Context

What the code does today (read before writing this, after finding M020), and what the compiler says, by the programs below.

**A field of a structure the function owns cannot move (F214).**

```peras
struct Row {
  country: String;
  amount: i32;
}

export fn take(text: String): String {
  const row = Row(text.copy(), 3);
  const country = row.country; // error[E0209]: cannot move a field out of its structure
  return country;
}
```

- **The ownership pass** (`perasc-check/src/owner.rs`) follows each path through a function with the variables moved on it, `Path.moved`, from a variable to where it moved. Paths are merged after an `if`, a `match` and a loop; a variable moved in one turn of a loop is moved at the start of the next. A field or an element consumed is refused (`E0209`) whatever holds it: a parameter that only lends it, an element of a `for…of`, or a local the function owns.
- **The lowering** already keeps every scalar of a structure in a local of its own, and follows each local that owns a buffer on every path (`live`). A structure is released leaf by leaf, at the end of its scope, before a `return`, and where paths meet. A field consumed is read today by borrowing its structure, then picking its leaves.
- So moving a field asks the checker to follow a part of a variable. For the lowering, it means moving a few of the structure's locals, which it already knows how to do for whole variables.
- The finding's cost: the dashboard copies the country a second time where it could take it, 7 ms of 322 per 1,000,000 rows.

**A function cannot be used as a value (F308).**

```peras
fn twice(x: i32): i32 {
  return x * 2;
}

export fn a(xs: Array<i32>): Array<i32> {
  return xs.map(twice); // owned: E0238, this method takes an arrow function
}                       // managed: E0210, expected `(i32) => void`, found `fn twice`

export fn b(n: i32): i32 {
  const g = twice;
  return g(n); // E0221 in both modes: `()` cannot be used on `fn twice`
}

fn apply(f: (i32) => i32, x: i32): i32 { // owned: E0242, a function type belongs to managed modules
  return f(x);
}

export fn c(n: i32): i32 {
  return apply((x) => x + 1, n); // owned: E0238, an arrow function is a value only in a managed module
}
```

- **The checker** gives a named function the type `fn name` (`Ty::Fn`), which nothing can call. A function type `(A) => R` (`Ty::Callback`) is written only in a managed module, or in an `extern` block. A value of that type is called in a managed module.
- **In a managed module**, `apply` and `c` are accepted. `xs.map(twice)` is refused because the type of `map`'s result is taken from an arrow function's body, and `twice` has none; this is a bug of the checker, found by writing this.
- **In an owned module**, an arrow function exists only as the argument of a method such as `map`. It is lowered in place, at the call (`lower/functional.rs`), its parameters bound to each element and its captures read from the enclosing locals. No function value exists at run time.
- **An exported function** that takes or gives a function is refused in both modes (`arrow_export`): JavaScript would call it with functions whose types go unchecked.

## Decision

### Part A: a field moved out of a structure (F214)

1. **What can move.** A field of a structure held by a variable the function owns can move: a `let` or a `const` declared in the function, or a `sink` parameter. It moves where a value is consumed: `const country = row.country;`, an argument to a `sink` parameter or a constructor, a `return`, `push`.
   - The path goes through fields only: `row.place.city` moves.
   - Through an element, an option, a `match` binding, the element of a `for…of`, a parameter that only lends, or a module variable, it is still refused (`E0209`). Those do not belong to the function.
2. **The checker follows moved places as it follows moved variables.** A place is a variable and a path of fields. It is followed on every path, merged after `if`, `match` and loops, with the rule of loops unchanged.
3. **After the move:**
   - The moved field, or a field within it, read or moved, is reported (`E0204`). The message names the field and where it moved.
   - The variable as a whole, read, moved, copied, passed, returned or matched, is reported too (`E0204`): it is partly moved, and the message names the field.
   - Its other fields stay usable.
   - Assigning the moved field (`row.country = …`) gives it back; assigning the variable gives back all of it.
4. **An arrow function** cannot move a field of a variable it captures, as it cannot move the variable.
5. **The lowering moves the field's locals**, which leave the set of live locals. The rest of the structure is released as today, leaf by leaf. The IR gains nothing; the release verifier proves each buffer released once on every path.
6. **A managed module** has no moves: nothing changes there.

### Part B: functions as values (F308)

1. **A named function is a value of type `(A, B) => R`** wherever its signature fits, in both modes:
   - passed to `map`, `filter` and the other methods that take a function: `xs.map(twice)`;
   - bound to a variable, passed as an argument, returned;
   - held in a field, an array or an option.
2. **A function type is written in an owned module too:** `(i32) => i32` as a parameter, a result, a variable, a field, an element or an option. `E0242` no longer applies to it.
   - An exported function that takes or gives one stays refused in both modes.
   - Equality on functions stays refused.
3. **A value of a function type is called:** `f(x)` when `f` is a variable or a parameter, and `(s.handler)(x)` through any expression.
4. **An arrow function is a value in an owned module when it captures nothing.** One that captures a variable is refused (`E0900`, `closure`); closures are a step of their own, still planned. A managed module keeps its arrow functions as they are.
5. **What cannot be a value:**
   - a generic function, unless its type arguments are given by the type expected;
   - a function with a `sink` or an `inout` parameter.

   A function type has no conventions: its parameters are lent and its result is the caller's. Each refusal is reported where the function is used as a value.

6. **In WebAssembly, a function value is an `i32`**, its index in the module's table of functions (`funcref`). A call is `call_indirect` with the type of its signature, which WebAssembly checks.
   - A function enters the table only when it is used as a value.
   - The IR gains `FnRef(FnId)`, an `i32`, and an indirect call.
   - The Rust backend writes the call as a `match` on the index over the functions of that signature, with no `unsafe`; the interpreter calls the function the index names.
7. **What the optimizations see.** `xs.map(twice)`, with `twice` named where it is passed, is lowered as `(x) => twice(x)`: a direct call, as fast as today. A call through a value is opaque to the optimizations, as any call is.

### Order

Part A lands first, then Part B, each with its criteria and its record. A part that cannot meet its criteria does not land, and the other is not held back by it.

## Not chosen

- **F308 by static resolution only.** `const g = twice` and `xs.map(twice)` would be resolved when compiling, with no function value at run time. It is smaller, but it leaves `fn apply(f: (i32) => i32, …)`, the use that makes functions values, to managed modules.
- **Closures**, arrow functions that capture and outlive their call. They need an environment that is allocated, owned and released, and their own decision.
- **A function crossing to JavaScript.** It needs the loader to wrap a JavaScript function into an index, and an index into a JavaScript function, with its types checked at each call.
- **For F214, an implicit copy, or a word of its own such as `take(row.country)`.** A copy is the cost the finding names. A move reads as a move of a variable already reads, in Peras as in Rust.

## Criteria, registered before any code

Each part lands only with all of its criteria, in addition to [decision 0021's criteria](0021-phases-4-and-5.md#criteria-of-each-step).

### Part A

1. **UI tests of the checker.**
   - Refused (`E0204`):
     - a field moved, then read;
     - a field moved, then the variable used whole;
     - a field moved on one branch only, then read after the `if`;
     - a field of a variable declared outside a loop, moved in it;
     - a field within a moved field, read.
   - Accepted:
     - another field of the variable, read after the move;
     - the moved field assigned, then read;
     - a field two structures deep moved.
   - Still `E0209`: the field of a lent parameter, of a `for…of` element, through an element, of a module variable, and in an arrow function's body.
2. **End to end, on both allocators.** The moved field and the rest of the structure are each released once, on every path, and nothing is left alive. The paths:
   - moved in one branch only;
   - moved in a loop's body, from a structure made in the turn;
   - an early `return`;
   - `?`;
   - `break`.

   The release verifier accepts each. The lowering, changed on purpose to release the moved field too, is refused by the verifier.

3. **The differential test** gains a statement that moves a field out of a structure made in the turn, on one branch, and passes 2,000 seeds. The same mutation as criterion 2 must fail it (finding M015).
4. **The dashboard.**
   - `bench/data/dashboard.peras` takes the country where it copied it a second time, and gives the same summary byte for byte (`node scripts/bench-data.mjs --check`).
   - Its A/B by `node scripts/ab-data.mjs` is reported, not a condition.

### Part B

5. **UI tests of the checker, in both modes.**
   - Accepted: each place a function value can go.
   - Refused:
     - a signature that does not fit (`E0210`);
     - a generic function with no type arguments;
     - a function with a `sink` or an `inout` parameter;
     - an arrow function that captures, as a value in an owned module (`E0900`);
     - an exported function with a function type;
     - `==` on functions.
   - The checker bug: `xs.map(twice)` is accepted in a managed module.
6. **End to end, on both allocators,** against the same module built as a managed one, which is JavaScript. The calls:
   - `apply(twice, x)`;
   - a table of functions in an array, called in a loop;
   - a function in a field and in an option;
   - a function returned by a function that chooses it by an argument;
   - `xs.map(twice)` and `xs.sortBy(key)`.

   The functions take and give strings and structures: the arguments are lent and the result taken through an indirect call. Nothing is left alive.

7. **The differential test** gains a dispatch through an array of functions and passes 2,000 seeds. Calling the function at the wrong index, on purpose, must fail it.
8. **The Rust backend and the verifier.** A unit test holds the Rust written for an indirect call. The release verifier takes the result of an indirect call as owned and its arguments as lent; a mutation that releases an argument inside it is refused.
9. **Size and cost.** Two figures are reported, not conditions:
   - the size of a module with and without a function used as a value;
   - the cost of a call through a value against a direct call, measured in one process (finding M017).
10. **Docs**, in English and in French: the guide's page on functions, the status (closures still planned), `E0242`, `E0238`, `E0221` and `E0900`, and findings F214 and F308.

## Record

_Each part is recorded here as it lands._

**Both parts: landed together**, as the owner asked, part B with real function values.

- **What changed, part A.**
  - The ownership pass follows moved places, a variable and the fields on the way, as it follows moved variables.
    - A field of a local, or of a `sink` parameter, moves.
    - The field, or what holds it, used after is reported (`E0204`), as is the structure used whole, with a new message for a structure partly moved.
    - Assigning the field gives it back.
    - The element of a `for…of`, which a loop lends, is told apart from a local of its own.
  - The lowering moves the field's locals, which leave the live set; the release verifier accepts every path.
  - The dashboard takes its country instead of copying it a second time.
- **What changed, part B.**
  - The checker:
    - writes function types wherever a type is, in both modes;
    - types a function of the module, not called, as one, unless it is generic or takes a parameter `sink` or `inout`;
    - gives a function by its value to `map` and the others (F010 fixed);
    - refuses `==` on functions, and an exported function that takes or gives one, even deep in a structure.
  - The IR gains three forms:
    - `FnRef`, a function as an `i32`;
    - `CallIndirect`;
    - `CallManyIndirect`.

    Every pass that matched a call was read and given them; the compiler listed each match that missed them.

  - The backends and the runtime:
    - WebAssembly has one `funcref` table, filled once by an element segment, and `call_indirect` with a type per signature;
    - the linker keeps the module's table and its segment, renumbered;
    - the Rust backend writes a `match` on the position;
    - the interpreter calls by position.
  - An arrow function used as a value becomes a function of its own, lowered after the others, its parameters lent. A name it does not declare is a capture, refused as `closure`.
  - `xs.map(twice)` calls `twice` directly: the value stands for the body of the loop.
- **Tests, each criterion in turn.**
  1. `E0204-moved-field.peras` and `E0209-field-not-owned.peras`: each refusal and each case allowed that the criterion names.
  2. `functions.test.ts`, on both allocators: a field moved in one branch, in a loop with `break` and `continue`, before an early `return`; nothing left alive. Moving the field without leaving the live set, on purpose, is refused by the release verifier: `holds a block on one path and not on another`.
  3. The differential test gains a field moved on one branch; it passes 2,000 seeds, and the same mutation fails it at seed 0.
  4. The dashboard gives the same summary (`bench-data.mjs --check`). Its A/B, `node scripts/ab-data.mjs HEAD --before-source <the dashboard that copies>`, 1,000,000 rows, 20 rounds: 310.3 ms before, 301.6 ms after, a ratio of 1.025 in the median, from 0.788 to 1.446. Reported, not a condition.
  5. `E0210-function-values.peras` and `.apeiron`, `E0238-function-export.peras`, `E0900-closure.peras`, and `function-values.peras`, whose IR is kept. The checker bug is fixed: `xs.map(twice)` is accepted in a managed module.
  6. `functions.test.ts`, on both allocators, against the same source built as a managed module:
     - `apply(twice, x)`;
     - an array of functions called in a loop;
     - a function in a field and in an option;
     - a function chosen by a function;
     - `map`, `filter`, `reduce` and `sortBy` given functions by name;
     - strings and structures given and taken, an owned string lent.

     The results are the same, and nothing is left alive.

  7. The differential test gains a dispatch through an array of functions and a function chosen by a call; it passes 2,000 seeds. Calling the next function of the table, on purpose, fails it at seed 11.
  8. `a_call_through_a_value_matches_on_the_table` holds the Rust written. `a_call_through_a_function_value_only_lends` holds the release verifier: a moved argument is refused, and the test fails without that rule.
  9. A loop calling a function 20,000,000 times, in one process, the order alternating:
     - the module is 143 bytes, or 173 with the function as a value: 30 bytes for the table, its segment and a type;
     - through the value, 77.7 ms against 76.5 ms directly, a ratio of 1.025 in the median, from 0.871 to 1.219.

     Reported, not conditions.

  10. The guide's pages on functions and memory, the status, `E0204`, `E0209`, `E0238` and `E0900`, and findings F214, F308 and F010, in English and in French.
- **Departures from the text above.**
  - Both parts land in one batch, as the owner asked, where the text above had A before B.
  - A generic function is never a value, even when the type expected would give its type arguments: inferring them there is left for later.
  - A function with a `sink` or `inout` parameter, used as a value, is reported as a mismatch (`E0210`), not with a message of its own.
  - Calling through a field is written `(s.handler)(x)`: `s.handler(x)` stays a method call.
  - In criterion 8, the mutation was made in the verifier's test rather than in the lowering. Lending is decided by the ownership pass, so a lowering changed alone does not move anything; changed with the ownership pass, the checker reports the move first.
  - The A/B of criterion 4 compares two versions of the dashboard with one compiler. `ab-data.mjs` gained `--before-source` for it.
