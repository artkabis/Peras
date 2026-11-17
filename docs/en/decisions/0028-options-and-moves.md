# 0028: Taking a value or leaving, and when a move happens

_French translation: [../../fr/decisions/0028-options-and-moves.md](../../fr/decisions/0028-options-and-moves.md)._

_Status: accepted by the owner on 2026-11-17, options 1b, 2e and 3b, after its options were measured by a protocol registered before any prototype was built, as the rule of the project asks ([decision 0027](0027-closures-over-the-heap.md) was the first). It answers findings F101, F103, F104 and F105, and the bug F012 found while reading the code for it._

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

## Decision

**Options 1b, 2e and 3b**, each the easiest to write and as fast as the fastest of its question.

1. **A move happens at the call** (1b). An argument that moves a variable moves it once every argument is evaluated: a later argument may read it, as long as it does not lend it to the same call. `m.set(key, m.get(key).unwrapOr(0) + 1)` builds; `f(key, key)`, the second lent, stays refused.
2. **An option is narrowed by a test that leaves** (2e). After `if (x.isNone()) { … }` whose block never ends normally, a `const` or a parameter not `inout` `x` is its value for the rest of the block: `x.qty`, `x * 2`, `f(x)`. Reading it reads the value the option holds, lent or moved as any variable's.
3. **A fallback is evaluated only when it is needed** (3b), in both modes, as JavaScript's `??` and `? :` do: `unwrapOr` runs its fallback in the branch where there is no value, whatever the fallback makes. This fixes F012.
4. The other options stay buildable behind `PERAS_PROTO_MOVE`, `PERAS_PROTO_TAKE` and `PERAS_PROTO_FALLBACK`, for the measure script only; without them, a build follows this decision.

## Criteria of the step, registered before its code

1. **UI tests**: the six programs of the protocol, as written, check with no error; `f(key, key)` with the second lent, narrowing a `let`, and narrowing after a block that does not leave stay refused, with their codes.
2. **End to end, on both allocators**, against the same source built as a managed module: the six programs, a narrowed option holding a string, read then moved, and narrowed in a loop; a move at the call into a map and into a function; a fallback with an effect, one that makes a string, and one that makes a temporary. The same results, nothing left alive.
3. **The differential test** gains a narrowed option, a lazy fallback with an effect on a module variable, and a key moved at the call; it passes 2,000 seeds.
4. **Each rule broken on purpose is caught**: moves applied as each argument is evaluated, in the release verifier, refuses the programs of criterion 2; a narrowed name reading the option whole, in the lowering, fails the end-to-end tests; the fallback run when there is a value, in the generator, makes the modes disagree and fails them.
5. **Measures**, reported, not conditions: `node scripts/bench-options.mjs` again at the commit of the step; the dashboard's A/B against the commit before (`ab-data.mjs`).
6. **Docs**, in English and in French: the guide's page on options, the methods reference for `unwrapOr`, the status, findings F012, F101, F103, F104 and F105, the roadmap, the changelog, `CLAUDE.md`. And what the audit of 2026-11-17 found stale: the README's status paragraph, the guide's introduction, two broken links; and a check of every link of the Markdown pages in `pnpm run check`.

## Record

_Each part is recorded here as it lands._

- **2026-11-17, the options measured** (`node scripts/bench-options.mjs`, [results](../../../bench/options/RESULTS.md); prototypes at `8857c01`, behind `PERAS_PROTO_MOVE`, `PERAS_PROTO_TAKE` and `PERAS_PROTO_FALLBACK`). An Intel Xeon at 2.30 GHz, 4 cores, Node 24, 21 rounds, owned modules on TLSF. Every prototype gives the managed module's results on every workload and program it accepts, and leaves nothing alive.
  - **Speed**, the median of the ratios to the option that keeps today's behaviour:

    | Workload                                    | Options                                                                               |
    | ------------------------------------------- | ------------------------------------------------------------------------------------- |
    | W1, words counted                           | 1a copied by the program 1; **1b at the call 0.981**; 1c copied for the program 1.474 |
    | W2, lines parsed, one in ten bad            | 2a 1; 2b 1.023; 2c 0.601; **2e narrowed 0.618**                                       |
    | W2, every line good                         | 2a 1; 2d `unwrap()` 0.576                                                             |
    | W3, a string as fallback, found nine in ten | 3a always 1; **3b when needed 0.463**; 3c `unwrapOrElse` 0.811                        |

    2a and 2b copy the value they take, from the option a variable keeps or from the arm that lends it; 2c, 2d and 2e do not. 1c's copy, inserted by the compiler, costs more than the same copy written in the source (1a); why was not looked into.

  - **Load on the processor**: the CPU time over the wall time is between 0.98 and 1.00 everywhere: no option changes it.
  - **Memory and bytes**: the same memory for every option of a workload; the modules differ by 90 bytes at most (2,932 to 3,029 gzipped).
  - **Ease of writing**, six programs written from JavaScript:

    |            | Programs that build as written | Errors met | Lines changed | Notions JavaScript does not ask for       |
    | ---------- | -----------------------------: | ---------: | ------------: | ----------------------------------------- |
    | 1a         |                         0 of 2 |          2 |             4 | `copy()` where a later argument reads     |
    | **1b**     |                         2 of 2 |          0 |             0 | none                                      |
    | 1c         |                         2 of 2 |          0 |             0 | none, but a copy unseen                   |
    | 2a         |                         0 of 3 |          5 |            11 | a placeholder never used                  |
    | 2b         |                         0 of 3 |          5 |            49 | `match` with an arm that leaves, `copy()` |
    | 2c         |                         0 of 3 |          5 |            24 | `const … else`                            |
    | 2d         |                         0 of 3 |          5 |            11 | `unwrap()`, which stops the program       |
    | **2e**     |                         3 of 3 |          0 |             0 | none                                      |
    | 3a, 3b, 3c |                         1 of 1 |          0 |             0 | none (3c offers `unwrapOrElse`)           |

  - **The modes agree**: today, a fallback with an effect gives `501, 202` owned and `500, 101` managed (F012); 3a, 3b and 3c each make them agree.
- **Departures from the protocol.**
  - **An option was added after measuring**: 2e, narrowing, as TypeScript does: after `if (x.isNone())` whose block leaves, a `const` or a parameter `x` is its value for the rest of the block. Every program of Q2 is written that way in JavaScript, and none of 2a to 2d accepted them.
  - **2c is prototyped for `Option` and `Result`; 2e for `Option` and `isNone()` only.** 2d stops the program as an index out of bounds does, a message to write if it is chosen.
- **What the measures say**: 1b, 2e and 3b are each the easiest to write, with no error and no change, and as fast as the fastest option of their question (1b and 1a, 2e and 2c, 3b alone). They are the options to propose to the owner, who chooses.
- **2026-11-17, the step: 1b, 2e and 3b by default** (`PERAS_PROTO_MOVE`, `PERAS_PROTO_TAKE` and `PERAS_PROTO_FALLBACK` now name another option for the measure script only, or `before`, the compiler before this decision). Its criteria:
  1. **UI tests**: the six programs as written check with no error (`tests/ui/check/options-as-written.peras`); `f(key, key)` with the second lent stays refused (`E0204-moved-at-call.peras`); narrowing a `let`, and after a block that does not leave, stay refused (`E0221-narrowing.peras`). Met.
  2. **End to end, on both allocators** (`packages/vite-plugin-peras/e2e/options.test.ts`), against the same source built as a managed module: the six programs, a narrowed string read then moved, narrowed in a loop, a move at the call into a map, a constructor and a function, the three fallbacks. The same results, nothing alive. Met.
  3. **The differential test** gains a narrowed string and number, a lazy fallback with an effect on a module variable, a key moved at the call into a map and a constructor; it passes 2,000 seeds. Met.
  4. **Each rule broken on purpose is caught**: (a) moves applied as each argument is evaluated, in the release verifier, refuses the end-to-end program (`movedAtCall`: "%27 is used after its block was released or moved"); (b) a narrowed name reading the whole option, in the lowering, fails the end-to-end tests (the verifier refuses the module); (c) the fallback run first, in the JavaScript generator, makes the modes disagree (`"5|one|5|0"` against `"5|one|5|11"`). Met, after a departure below.
  5. **Measures**, at `ad1ff36`, the same machine as the options measured (an Intel Xeon at 2.30 GHz, 4 cores, Node 24), 21 rounds:
     - `node scripts/bench-options.mjs` ([results](../../../bench/options/RESULTS.md)), each option against the compiler before the decision: 1b 0.980, 2e 0.625, 3b 0.456, as when the options were measured (0.981, 0.618, 0.463); the processor's load 1.00 and the memory the same everywhere. The modes now agree on a fallback with an effect, `500, 101` in both; before the step, `501, 202` and `500, 101`. The managed module is faster than every owned one on W2 and W3 (0.12 to 0.36 of the time): both build and walk strings, which an owned module copies into its memory; entered as finding F220.
     - `node scripts/ab-data.mjs cbe3981`, the dashboard against the commit before the step, 20 rounds: 0.996 round by round, from 0.936 to 1.151. No change: the dashboard's fallbacks were already quiet (F209).
  6. **Docs**: the guide's options and memory pages, the methods reference, the status, the roadmap, the changelog, the findings (F012, F103, F104, F105 closed, F101 partly), `CLAUDE.md`; the README's status paragraph and the guide's introduction, stale since decision 0009; the two broken links fixed, and `scripts/check-links.mjs` in `pnpm run check`, which checks every link of the 325 Markdown pages, file and heading. Met.
- **Departures from the criteria.**
  - **Mutation (a) first passed** (finding M027): no program of the end-to-end tests had a later argument that reads the moved value in place. A constructor, `Sized(text, text.length)`, and a call to a function were added; the constructor was refused by the release verifier without the mutation, a bug of the step (F013), fixed: the numbers that follow a move in a constructor, `Some`, `Ok` or `Err` are computed first.
  - **Narrowing broke the programs written before it** (F014), found by the end-to-end tests of `examples/stream`: `parsed.unwrapOr(…)` after `if (parsed.isNone()) { continue; }`. On a narrowed name, the methods of `Option` read the option whole; the decision did not say so.
  - **The end-to-end test builds its managed module without `sink`**, which a managed module refuses (`E0243`) and which changes nothing there, since nothing moves.
  - **The differential test's 2,000 seeds were run in the release build** (66 s); its default seeds in the debug build the CI runs (70 s, finding M022).
