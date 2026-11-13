# 0023: Module variables in owned modules

_French translation: [../../fr/decisions/0023-module-variables.md](../../fr/decisions/0023-module-variables.md)._

_Status: proposed on 2026-11-12, with its criteria registered before any code; accepted by the owner the same day; built, its criteria met (see [Record](#record)); step 4 of [decision 0021](0021-phases-4-and-5.md), finding F306._

## Context

A `let` at the top of a module is a variable that the module's functions share, and it keeps its value from one call to the next. A managed module (`.apeiron`) compiles one to a JavaScript `let`. An owned module (`.peras`) checks one but does not compile it: `E0900` ("module-level constants and variables"). A module `const` is compiled: its value is written where it is used.

What the code does today (read before writing this, after finding M020):

- **The checker** accepts a module `let` of any type, with any initial value. It refuses moving its value out (`E0209`), so a function can only read it, lend it or assign it.
- **The IR** has locals and no other place to keep a value. Each optimization of decisions 0016 to 0018 reasons about locals: a value it has not seen written in a function is taken to be unchanged.
- **The WebAssembly backend** has no global of its own besides the runtime's. The Rust backend writes functions only.
- **A `parallel for`** runs its turns on threads, each thread an instance of the module with its own globals ([decision 0007](0007-threads-over-shared-memory.md)).

## Decision

1. **A first version for numbers and booleans.** A module `let` holds an integer, a float or a boolean, or a structure, an enumeration or an option made of them. In WebAssembly, each scalar is a mutable global.
   - A string, an array, a map or a set in a module variable stays refused (`E0900`, with that reason).
   - Holding such a value is more than storage. The memory a variable keeps would be alive between calls, so the debug allocator's leak check, which expects nothing alive after a call, would have to count it as a root. That is its own step.
2. **Its initial value is a constant**, under the rule a module `const` already follows (literals, other constants, operators on them). It is written into the module and set before any call. No code runs when the module loads, and the order between variables never matters.
3. **Its value lives as long as the module's instance.** Two pages, or two workers, each have their own, as JavaScript module variables do.
4. **A `parallel for` cannot read or write one** (`E0900`, with that reason). Each thread has its own copy of the globals, so a turn would see another value than the code around the loop.
5. **The optimizations take a module variable as changed by any call and any assignment to it.**
   - In the IR, reading one is an expression of its own, and writing one is a statement of its own; neither is a local.
   - A pass that does not know them leaves the code that touches them as written. The verifiers check that this holds.

## Not chosen

- **Strings and arrays now.** They would need the leak check to know roots, and the release of the old value on each assignment to be proven across calls. Both are possible, and both are their own step.
- **Any initial value, run when the module loads.** It needs an order between variables, and a call made before any function is called, which can trap before the page has a module. Constants cover counters, flags, seeds and bounds.
- **Sharing a variable between the threads of a `parallel for`.** It is a data race unless it is atomic, and Peras has no atomics.

## Criteria, registered before any code

The step lands only with all of these, in addition to [decision 0021's criteria](0021-phases-4-and-5.md#criteria-of-each-step):

1. **Values that persist.** An end-to-end test, on both allocators: a counter, a float sum and a boolean flag, each kept across calls from JavaScript, and a structure of numbers assigned whole and by field. The results match JavaScript.
2. **Each refusal has a UI test:** a string or an array in a module variable, an initial value that is not constant, and a `parallel for` that reads or writes one.
3. **The differential test** gains a module variable, read and written in its loops and in a function it calls, and runs with 2,000 seeds. Breaking on purpose the rule that a call may change a module variable fails it, for example by letting a value read before a call be reused after it.
4. **Both backends**, direct WebAssembly and Rust, give the same results (the "backends agree" job).
5. **Docs:** the guide's module constants become module constants and variables, and the status and `E0900` are updated, in English and in French.

## Record

_Each part is recorded here as it lands._

**Module variables of numbers and booleans: landed.**

- **What changed.**
  - The IR gains `Global`, a read, and `SetGlobal`, a write, and the module a list of its variables, each with its type and constant initial value. In WebAssembly each is a mutable global; in the Rust backend, a cell of its thread, with no `unsafe`; in the interpreter, a value of the machine.
  - The initial value is lowered, then run by the interpreter of the IR, so that its arithmetic, overflow included, is the module's. The checker holds it to the rule of module constants (`E0233`) in an owned module.
  - Every pass was read for what a module variable changes. `is_simple` takes a read as never movable, so neither the hoisting out of loops nor the unrolling of recursion moves it past a call. The vectorizer refuses a loop that reads or writes one. The release verifier refuses a block stored in one.
  - A variable refused where it is declared is still known, so its uses do not report the cause again.
- **Tests, each criterion in turn.**
  1. `globals.test.ts`, on both allocators: a counter, a float sum, a flag, a structure assigned whole (its fields swapped) and by field, and a loop calling a function that changes a variable it reads, against the same module in JavaScript; a second instance starts from the initial values.
  2. UI tests: a string in a module variable (`E0900`, `global-heap`), an initial value that calls a function (`E0233`), a `parallel for` reading one (`E0241`, `global`). The IR's own test of unsupported constructs covers one passed `inout` (`global-inout`).
  3. The differential test gains `tally`, changed by `bumped`, which a loop of `spun`, a function that uses no memory, calls between two reads. It passes 2,000 seeds, the two files version included. Letting a read of a module variable leave its loop, on purpose, fails it at seed 6.
  4. Both backends: the same calls, built by `perasc build --backend rust` and by the direct backend, gave the same results, `22 24 1.75 2.75 true false 13 23`. This was run by hand: the CI's "backends agree" job measures `bench/backends/kernels.peras`, which has no module variable, and a test that runs `cargo` and `wasm-bindgen` would be its own step. A unit test holds the Rust that the backend writes for them, with no `unsafe`.
  5. The guide's page of values gains module variables; the status, `E0241`, `E0900` and `E0233` are updated.
- **Departures from the text above.**
  - A `parallel for` that reads or writes a module variable is refused with `E0241`, reason `global`, where every refusal of a parallel loop is, rather than with `E0900` as written above. Its message says why: each thread has its own copy.
  - A module variable passed `inout` is refused (`E0900`, `global-inout`), a case the text above did not name.
