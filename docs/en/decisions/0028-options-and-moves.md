# 0028: Taking a value or leaving, and when a move happens

_French translation: [../../fr/decisions/0028-options-and-moves.md](../../fr/decisions/0028-options-and-moves.md)._

_Status: proposed on 2026-11-16, with its options and its protocol registered before any prototype is built or measured, as the rule of the project asks ([decision 0027](0027-closures-over-the-heap.md) was the first). It answers findings F101, F103, F104 and F105, and the bug F012 found while reading the code for it._

## Context

What the code does today, read and run before writing this (finding M020).

- **F103, a move takes effect where its argument is written.** `m.set(key, m.get(key).unwrapOr(0) + 1)` is refused (`E0204`): `key` moves into `set` by the first argument, and the second reads it after. The second argument's read ends before `set` is called; JavaScript accepts the line, and it is the most common line of a count.
- **F101, an arm that leaves gives the `match` no type.** In `const row = match (parse(line)) { Some(r) => r, None => { continue; } };`, the arm `None` is a block that never ends normally, but it is typed `void`, so the `match` is `void` and its use is refused with `` `void` has no field or method `qty` `` (`E0215`), which does not say why.
- **F105, no way to take a value or leave.** There is no `unwrap` and no `let … else`: each optional value takes a test, `isNone()`, then `unwrapOr` with a placeholder never used.
- **F104, the fallback of `unwrapOr` is evaluated when it is not needed**, in an owned module: a fallback that makes a string allocates at each call. Decision 0016 made a quiet fallback lazy (F209), not one with an effect.
- **F012, a bug: the two modes disagree on `unwrapOr`.** A managed module compiles it to `x !== undefined ? x : fallback`, which evaluates the fallback only when needed; an owned module evaluates it always. With a fallback that changes a module variable, the same source gives `501, 202` in an owned module and `500, 101` in a managed one.

## Options

Three questions, each answered by one of its options. Each option is built as a prototype where it can be, and measured by the protocol below before one is chosen.

**Q1, when a move happens (F103).**

- **1a.** As today: refused, with the fix `key.copy()`.
- **1b. A move happens at the call**, once every argument is evaluated: a later argument may read the value, as long as what it reads is not lent to the same call. `m.set(key, m.get(key).unwrapOr(0) + 1)` builds; `f(key, key)` with the second lent stays refused.
- **1c. A copy made for the program**: the compiler inserts `key.copy()` where a later argument reads what moves.

**Q2, taking a value or leaving (F101, F105).**

- **2a.** As today: `isNone()`, a placeholder, `unwrapOr`.
- **2b. An arm that never ends normally has no type** of its own (`never`): `match (parse(line)) { Some(r) => r, None => { continue; } }` has the type of `r`. When every arm leaves, the message says so.
- **2c. 2b, and `const … else`**: `const row = parse(line) else { continue; };` takes the value of `Some` or `Ok`, or runs the block, which must leave.
- **2d. `unwrap()`**, which stops the program with an error when there is no value.

**Q3, when a fallback is evaluated (F104, F012).**

- **3a. Always, in both modes**: the managed module evaluates it first too.
- **3b. Only when needed, in both modes**, as JavaScript's `??` and `? :` do: the owned module evaluates the fallback in the branch where there is no value.
- **3c. Always, and `unwrapOrElse(() => …)`** for a fallback evaluated only when needed.

## Protocol, registered before any prototype

Four quantities, as for decision 0027: speed, load on the processor, memory and bytes, ease of writing.

1. **Ease of writing**: six programs, written as a user would write them from JavaScript, then fixed until they build, for each option of the question they touch:
   - P1, words counted in a map, `m.set(w, m.get(w).unwrapOr(0) + 1)`;
   - P2, lines parsed into rows, a bad line skipped;
   - P3, a function that returns early when a lookup finds nothing;
   - P4, a lookup with a string as its fallback;
   - P5, a chain of three optional steps, each depending on the last;
   - P6, rows grouped by a key into a map of structures.

   For each: the errors met before it builds, with their codes, the lines changed from the JavaScript, the notions JavaScript does not ask for. Every fixed program gives what the same source gives built as a managed module.

2. **Workloads**, owned modules on TLSF in Node, every option's module in one process, the order alternating, 21 rounds, the median of the ratios to the option that keeps today's behaviour (1a, 2a, 3a), and their range (finding M017):
   - **W1**, P1 over 1,000,000 words drawn from 1,000: 1b against 1c, which copies the key at each word;
   - **W2**, P2 over 1,000,000 lines, one in ten bad: 2a, 2b, 2c and 2d (2d on lines all good);
   - **W3**, P4 over 1,000,000 lookups that find their key nine times in ten, the fallback a string made at each call: 3a, 3b and 3c.
3. **Load on the processor**: the CPU time over the wall time of each module's rounds.
4. **Memory and bytes**: the module's memory after each workload; its bytes, raw and gzipped.
5. **The modes agree**: for Q3, a fallback with an effect gives the same result in both modes for every option kept; an option that cannot make them agree is out.
6. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

## Criteria of the step that follows the choice

Set once the options are chosen, with the owner, as for decision 0027.

## Record

_Each part is recorded here as it lands._
