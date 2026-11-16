# 0027: Closures that capture strings, arrays and function values

_French translation: [../../fr/decisions/0027-closures-over-the-heap.md](../../fr/decisions/0027-closures-over-the-heap.md)._

_Status: proposed on 2026-11-16, with its options and its protocol registered before any prototype is built or measured; the first decision chosen by measuring its options, as the owner asked the same day. It follows [decision 0026](0026-closures.md), whose refusal `closure-heap` it lifts._

## Context

What the code does today, read before writing this (finding M020).

```peras
fn label(prefix: String): (String) => String {
  return (s: String): String => `${prefix}${s}`; // E0900: closure-heap
}
```

- **An environment holds `i64` slots**, numbers only, written when the closure is made and never after. It is counted: a copy of the function value adds one, the last release frees it ([decision 0026](0026-closures.md)).
- **A `const` cannot be changed, deeply**: `const xs = []; xs.push(1);` is `E0205`. A captured `const` string or array cannot change while the closure lives, unless it is moved away first.
- **Strings, arrays, maps and sets have one owner** and move ([decision 0003](0003-runtime-proved-at-compile-time.md)); a copy is written `copy()`. Nothing on the heap is shared today but environments.
- **The ownership pass reads an arrow function's body twice**, as the body of a loop, and refuses a move in it.
- **A managed module captures by reference**, as JavaScript does.

## Options

Each is built as a prototype where it can be, and measured by the protocol below before one is chosen.

- **A. Move into the environment.** Making the closure moves what it captures into it: the variable cannot be used after (`E0204`), unless the program writes `s.copy()` itself. The environment owns its buffers and releases them with its last holder. No copy is made in secret.
- **B. Copy into the environment.** Making the closure copies what it captures, as JavaScript would read it, since a `const` cannot change: the variable stays usable, and the cost is one copy per closure made.
- **C. Share by count.** The captured buffer is counted, like an environment: the closure and the variable hold the same one. It needs a count on every buffer, `Rc<T>`, finding F310.
- **D. Lend.** The closure borrows what it captures; it may be called and passed down, but not returned, stored, or kept beyond the scope of what it borrows. No copy and no count; an escaping closure is refused.
- **E. Lend, else move.** D where the closure does not escape, A where it does.

## Protocol, registered before any prototype

Four quantities, as the owner asked: speed, load on the processor, memory and bytes, and ease of writing.

1. **Workloads**, in an owned module built on TLSF, run in Node, each option's module built from the same source where the option accepts it:
   - **W1, made then called once**: a closure capturing a string of 16 bytes, 1 KiB and 64 KiB, made, called once and dropped, 1,000,000, 100,000 and 2,000 times.
   - **W2, called often**: one closure capturing a string of 1 KiB, called 20,000,000 times, each call reading one byte of it.
   - **W3, escaping**: `label(prefix)` returned by a function, then given to `map` over 100,000 strings of 10 bytes.
   - The same source built as a managed module, which is JavaScript, is the reference.
2. **Speed**: the time of each workload, every option's module in one process, the order alternating round by round, 21 rounds, compared by the median of the ratios to option B's, and their range (finding M017).
3. **Load on the processor**: the process's CPU time (`process.cpuUsage`, user and system) over the 21 rounds of each module, divided by the wall time: the cores kept busy, as decision 0012 measures a page.
4. **Memory and bytes**: the peak of the module's memory (`memory.buffer.byteLength`) and the most blocks alive (`__perasHeap`) during each workload; the module's bytes, raw and gzipped.
5. **Ease of writing**: five programs, written for each option as a user would write them, starting from the JavaScript a user would write, then fixed until they build:
   - a label maker, `label(prefix)` returned;
   - a filter by a captured array of words;
   - a closure kept in a structure with a captured name;
   - callbacks stored in an array, each capturing its own string;
   - a closure capturing a value the function still uses after.

   For each: lines changed from the JavaScript, notions a user must learn that JavaScript does not ask for, and errors met before the program builds, with their codes.

6. **An option not built** is reported as such, with why, and ranked on nothing measured (finding M001).
7. **Correctness first**: every prototype passes the end-to-end tests of decision 0026 and the new workloads' results against the managed module, with nothing left alive, before its times count.

## Criteria of the step that follows the choice

Set once an option is chosen, with the owner: the UI tests, end to end on both allocators, the differential test, the release verifier, the docs, as for decision 0026.

## Record

_Each part is recorded here as it lands._
