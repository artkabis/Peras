# 0027: Closures that capture strings, arrays and function values

_French translation: [../../fr/decisions/0027-closures-over-the-heap.md](../../fr/decisions/0027-closures-over-the-heap.md)._

_Status: accepted by the owner on 2026-11-16, option F, after its options were measured by a protocol registered before any prototype; the first decision chosen by measuring its options, as the owner asked the same day. It follows [decision 0026](0026-closures.md), whose refusal `closure-heap` it lifts._

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

## Decision

**Option F**, found by measuring the others: what a closure captures on the heap is copied into its environment, which owns it, unless the closure cannot escape, and then it is lent.

- A closure cannot escape when it is given in place as an argument, or kept in a `const` that is only called or given as an argument, and nothing it captures moves while it lives. The ownership pass decides it, function by function, and lowering lends or copies.
- A function value captured follows the same rule: lent where the closure cannot escape, held once more where it escapes, as a copy of it is ([decision 0026](0026-closures.md)).
- The program is written as option B asks: no new notion, no new error. The refusal `closure-heap` goes.
- The prototype switch `PERAS_PROTO_CLOSURE` stays, for the measure script only; without it, a build is option F.

## Criteria of the step, registered before its code

1. **UI tests**: `closures.peras` gains a closure lending a string, one copying it because it escapes, one copying it because what it captures moves after, and a function value captured; their IR is kept. `E0900-closure.peras` goes, its refusal with it.
2. **End to end, on both allocators**, against the same source built as a managed module: the five programs of the protocol, a closure capturing a map, a set, an array of strings and a structure holding one, and a closure capturing a closure. The results are the same, and nothing is left alive.
3. **Lending is safe**: lending what an escaping closure captures, on purpose in the lowering, fails the end-to-end tests (a closure reading a buffer already released); copying on purpose where it lends changes no result, and the measure shows its cost.
4. **The differential test** gains closures capturing strings, one escaping into an array, one only called, and passes 2,000 seeds. Reading the wrong handle slot, on purpose in the WebAssembly generator, fails it.
5. **The count holds**: an environment that owns buffers releases them with its last holder; making it own none, on purpose, leaves blocks alive, which the tests catch.
6. **Measures**, reported, not conditions: `node scripts/bench-closures.mjs` again at the commit of the step, F against the prototypes; the dashboard's A/B against the commit before (`ab-data.mjs`).
7. **Docs**, in English and in French: the guide's page on functions, the status, `E0900`, the CLI reference if it changes, finding F308's follow-up, the roadmap, the changelog, `CLAUDE.md`.

## Record

_Each part is recorded here as it lands._

- **2026-11-16, the options measured** (`node scripts/bench-closures.mjs`, [results](../../../bench/closures/RESULTS.md); prototypes at `39743d8`, behind `PERAS_PROTO_CLOSURE`). An Intel Xeon at 2.10 GHz, 4 cores, Node 24, 21 rounds, owned modules on TLSF. Every prototype gives the managed module's results on every workload it accepts, and leaves nothing alive.
  - **Speed**, the median of the ratios to B round by round:

    | Workload                  | A, move | B, copy |         D, lend | F, copy unless lent | managed |
    | ------------------------- | ------: | ------: | --------------: | ------------------: | ------: |
    | W1, 16 B, 1,000,000 times |   1.031 |       1 |           0.357 |               0.354 |   0.299 |
    | W1, 1 KiB, 100,000 times  |   1.006 |       1 |           0.205 |               0.202 |   0.075 |
    | W1, 64 KiB, 2,000 times   |   1.011 |       1 |           0.106 |               0.105 |   0.004 |
    | W2, 20,000,000 calls      |   0.995 |       1 |           0.992 |               0.989 |   1.659 |
    | W3, escaping              |   0.986 |       1 | not expressible |               1.002 |   0.816 |

    A costs what B costs: its programs copy where they would reuse what they captured, and a move saves nothing where the closure is made once. Lending saves the copy wherever a closure is made often, by 3 to 10 times. Called often, all are equal, and 1.66 times as fast as JavaScript.

  - **Load on the processor**: the CPU time over the wall time is between 0.98 and 1.12 for every owned option: one core, nothing more. No difference between the options.
  - **Memory and bytes**: the module's memory differs only for W1 at 64 KiB: 320 KiB for A, 256 for B, 192 for D and F, the copies needing room. The modules differ by 16 bytes at most (1,922 to 1,931 gzipped), 3.7 times JavaScript's, the allocator's cost.
  - **Ease of writing**, five programs written from JavaScript:

    |                                      |                 A, move |  B, copy |                      D, lend | F, copy unless lent |
    | ------------------------------------ | ----------------------: | -------: | ---------------------------: | ------------------: |
    | Programs that build as written       |                  2 of 5 |   4 of 5 |                       2 of 5 |              4 of 5 |
    | Errors met                           |                       4 |        1 |                            4 |                   1 |
    | Lines changed to build               |                      11 |        2 |                           25 |                   2 |
    | Programs rewritten without a closure |                       0 |        0 |                            3 |                   0 |
    | Notions JavaScript does not ask for  | `sink`, a capture moves | none new | a closure that cannot escape |            none new |

    The one error of B and F, in P3, is a rule Peras has without closures: a borrowed parameter put into a structure needs `copy()` (`E0209`).
- **Departures from the protocol.**
  - **An option was added after measuring**: F, B's copy skipped where the closure cannot escape, chosen by D's escape analysis instead of refused. The first run, of A, B and D only, is kept in [bench/closures/archive](../../../bench/closures/archive/run-1-without-f.md); its figures agree with the second.
  - **C and E are not built.** C needs a count on every buffer, `Rc<T>` (finding F310), a change of every buffer operation, not a prototype. E is left for F, which differs from it only where a closure escapes, copying where E moves; its figures are not inferred from the others.
  - **W2 reads the length and the first two bytes** (`startsWith`) rather than one byte at an index: no method reads one.
  - **Memory is the module's memory after each workload**, which only grows: the most blocks alive at once needs a counter the allocator does not keep.
- **What the measures say**: F writes as B, the easiest, and runs as D, the fastest, where D can run at all. It is the option to propose to the owner, who chooses.
