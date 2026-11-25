# 0046: What to build next, measured again

_French translation: [../../fr/decisions/0046-what-to-build-next-again.md](../../fr/decisions/0046-what-to-build-next-again.md)._

_Status: protocol registered on 2026-11-25, before any program is written, as the rule of the project asks; programs, measure and ranking recorded the same day, the owner's decision recorded the same day. Asked by the owner on 2026-11-25: decide what comes next by measure, as [decision 0011](0011-what-to-build-next.md) did, rather than by the order of the [roadmap](../reference/roadmap.md)._

## Context

The last time what to build was ranked by measure was decision 0011, thirty-four decisions ago: seventeen bugs of web applications and a case study written from TypeScript. Since then the decisions have followed the roadmap's order, and three in a row kept what was there: arenas (0043) and pools (0044) were measured and not kept, and the first verdict of 0045 too. Meanwhile the one friction found by writing programs as a user would, an option not narrowed through `&&` and `||` (finding F118), was worth a decision of its own: eight errors on five programs, none since 0045.

Frictions known and not yet ranked, each from a finding: JSON in owned modules (`E0900`), a `let` never narrowed, the reads of a field the tools cannot resolve (F313), a failing test that cannot show an array or a structure (F227), no allocation in a turn of a `parallel for` (F301), module variables that cannot be strings or arrays (decision 0023), packages (phase 5). And the next step of the roadmap, the `@frame` rule.

## Method, registered before any program is written

### The programs

Four applications, each written first in TypeScript, the specification, then in Peras by translating it as a developer coming from TypeScript would, without reshaping it to fit what Peras offers. They are chosen to cover both modes and the kinds of work the roadmap names, and none of them is a program the repository already has:

- **A1, notes** (a managed module): a list of notes kept in the page's storage as JSON, searched as the user types, sorted by date, with tags; the DOM, events, timers, storage, JSON.
- **A2, a ledger** (an owned module called from a managed one): a CSV of transactions read, grouped by month and by category, with a running balance and the largest movements; strings, maps, sorting, numbers of money.
- **A3, a particle scene** (an owned module): a frame of particles that are born, move, collide on a grid and die, drawn into a framebuffer; arrays of structures, the work of a frame, what `@frame` would rule.
- **A4, a route finder** (an owned module): the shortest path between stops of a small network read from JSON, with options for a stop that is missing; structures that point to one another, options, a priority queue.

### What is counted, for each program

1. **Errors met**: each run of `perasc check --json` on the Peras version as written, then after each fix, until it builds; every diagnostic recorded with its code, its message, and the fix made. A fix is the smallest change a user would make from the diagnostic.
2. **What cannot be written**: a part of the TypeScript the Peras version cannot express at all, recorded with what was done instead (`E0900`, a missing method, a missing type).
3. **The lines**: of each version, blank lines and comments apart, and the lines the Peras version changes from a literal translation.
4. **The results**: each Peras version gives what its TypeScript gives, on the same inputs, built for production (`--allocator tlsf`, finding M029).
5. **The time**, secondary: each version's time on its workload against its TypeScript's, in one process, the order alternating round by round, 21 rounds, with an A/A control, the TypeScript timed against itself in the same rounds (finding M042).

### The ranking

Each error, each thing that cannot be written and each changed line is attributed to one cause: an existing finding, a step of the roadmap, or a new finding. A cause is scored by the programs it blocks (a part that cannot be written), then by the errors it causes, then by the lines it adds. The ranked list is the order of the next decisions; a cause that no program meets is not ranked, and says so.

### What this cannot show, said now

The programs are written by the authors of Peras, who know what it refuses: they cover the applications the authors know, not every application, as decision 0011 said of its corpus. To bound that: each TypeScript version is written and committed before its Peras version, and not changed after; each fix is the one the diagnostic offers, or the smallest a user would make, recorded as such.

## Record

### The specifications (2026-11-25, before any Peras)

The four programs in TypeScript, `bench/next/{notes,ledger,particles,routes}.ts`, type-checked at TypeScript's strictest (`bench/next/tsconfig.json`: `strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`), each with a `run` that makes its own input and gives a line of text to compare: A1 for 2,000 notes gives `2000|139|1986|books:800,…`; A2 for 20,000 rows, twelve months and a lowest balance of -382,191.29; A3 after 300 frames, 1,792 particles and a digest of the pixels, 694335550; A4 on a grid of 60 by 60 stops, `298:119|330:121|missing nowhere`. Written before this commit, the ledger's generator was corrected twice, before any line of Peras: its low bits made five months empty, and a salary of 2,500 kept the balance from ever falling below zero; it takes the generator's high bits, and a salary of 300. From this commit on, they are not changed.

### The Peras versions (2026-11-25)

Each TypeScript translated as registered, `bench/next/{notes.apeiron,ledger.peras,particles.peras,routes.peras}`, every run of `perasc check --json` kept with its source and the fix made, in [`bench/next/ease`](../../../bench/next/ease). Each builds for production and gives its TypeScript's line, twice on one instance (`node scripts/bench-next.mjs`).

| Program              | Runs | Errors met | Cannot be written                                                                | Bugs of perasc found |
| -------------------- | ---: | ---------: | -------------------------------------------------------------------------------- | -------------------- |
| A1, notes            |    4 |         23 | –                                                                                | F123                 |
| A2, a ledger         |    6 |         25 | –                                                                                | –                    |
| A3, a particle scene |    7 |         38 | a field of `inout self` given a new array (F121); a framebuffer of clamped bytes | F121, F122           |
| A4, a route finder   |   10 |         29 | JSON read in an owned module (E0900, F120)                                       | F119, F120           |

115 errors in all. The first commit of A3 said 36 and that of A4 21; the counts above are read from the runs themselves, and those two were wrong.

Two departures from the protocol, said here. **The declarations were written in Peras's forms**, `struct`, `impl`, `enum` and `while (true)` for TypeScript's interfaces, classes, unions and `for (;;)`, as the guide teaches first: a developer who has not read it would meet more errors than these, so the count is a floor. **Four results differed** from TypeScript's once a version built, with no diagnostic: three are numbers (A2, a product above 2⁵³ that JavaScript rounds and an `i64` does not; A3, an integer division whose quotient met an integer, and a sum that wraps an `i32` in production), one is an element read into a `let` and changed as a copy, where E0205's fix led (F122, its message corrected). They are counted with their cause below, as results changed in silence, weighed above any error.

### The ranking

Each error attributed to one cause, from the runs; the score as registered, by the programs a cause blocks, then by its errors.

| Rank | Cause                                                                                                      | Programs blocked |        Errors (A1, A2, A3, A4) | Results changed in silence | Where it stands                                                                          |
| ---: | ---------------------------------------------------------------------------------------------------------- | ---------------- | -----------------------------: | -------------------------: | ---------------------------------------------------------------------------------------- |
|    1 | JSON in an owned module                                                                                    | A4               | 0 by `check`, E0900 by `build` |                          – | finding F120, open                                                                       |
|    1 | A field of `inout self` given a whole value                                                                | A3               | 0 by `check`, E0900 by `build` |                          – | finding F121, the error landed, the feature open                                         |
|    1 | A framebuffer, bytes clamped as `Uint8ClampedArray`                                                        | A3               |                 1 (0, 0, 1, 0) |               1 (rounding) | roadmap, phase 6, `Framebuffer<Rgba8>`                                                   |
|    4 | A JavaScript number used as an integer and as a float                                                      | –                |               31 (1, 7, 22, 1) |                          3 | decision 0030, 0034; new                                                                 |
|    5 | A value moved out of an element, a field or a binding                                                      | –                |                17 (0, 7, 2, 8) |                          – | by design (decision 0025); its fix is offered                                            |
|    6 | Syntax of TypeScript absent: spread, destructuring, `?:`, `?.`                                             | –                |                14 (4, 7, 0, 3) |                          – | new                                                                                      |
|    7 | The browser typed with Peras's own names                                                                   | –                |               14 (14, 0, 0, 0) |                          – | decision 0009                                                                            |
|    8 | An element read into a variable is a copy                                                                  | –                |               10 (0, 0, 10, 0) |                          1 | F122; Hylo's projections, in [the research](../research.md#changing-an-element-in-place) |
|    9 | Globals and methods of JavaScript absent: `Number`, `String`, `padStart`, `Infinity`, `Map.values`, `fill` | –                |                 8 (0, 4, 2, 2) |                          – | new                                                                                      |
|   10 | An arrow's parameters and its empty results not typed from where it is used                                | –                |                 7 (0, 0, 0, 7) |                          – | new                                                                                      |
|   11 | A `let` not narrowed                                                                                       | –                |                 5 (0, 0, 1, 4) |                          – | decision 0045, said then                                                                 |
|   12 | No sort by a comparator                                                                                    | –                |                 3 (3, 0, 0, 0) |                          – | decision 0011, `sortBy`                                                                  |
|   13 | A `match` arm that assigns                                                                                 | –                |                 2 (0, 0, 0, 2) |                          – | new                                                                                      |
|    – | As designed: `pop()` and `fromJson` give an option and a result                                            | –                |                 2 (0, 0, 0, 2) |                          – | not a friction                                                                           |
|    – | A bug of perasc, F123                                                                                      | –                |                 1 (1, 0, 0, 0) |                          – | fixed                                                                                    |

Not ranked, because no program met them, as registered: packages (each program is one file), `@frame` (A3's frame is a loop over its particles, written without it and correct), allocation in a turn of a `parallel for` (F301), module variables of strings or arrays, the tools' reads of a field (F313), a failing test that shows an array (F227).

### The measure (2026-11-25)

`node scripts/bench-next.mjs`, 21 rounds in one process, each Peras version built for production; [`bench/next/RESULTS.md`](../../../bench/next/RESULTS.md).

| Program                   | Lines, TypeScript | Lines, Peras | Peras / TypeScript, median (range) | A/A, median (range) |
| ------------------------- | ----------------: | -----------: | ---------------------------------- | ------------------- |
| A1, notes, 2,000          |                93 |          114 | 5.80 (2.95 to 7.08)                | 1.02 (0.93 to 5.07) |
| A2, a ledger, 20,000      |                65 |           92 | 1.59 (1.40 to 1.76)                | 1.04 (0.90 to 1.60) |
| A3, a particle scene, 300 |                87 |          120 | 3.28 (2.53 to 3.67)                | 1.04 (0.76 to 1.38) |
| A4, a route finder, 60    |               118 |          166 | 0.66 (0.40 to 0.85)                | 1.14 (0.64 to 2.23) |

The A/A control's medians lie between 1.02 and 1.14, its ranges up to 5.07: a median inside that band decides nothing (finding M042), and none of the four is inside it. The first run of this measure gave A1 186 times TypeScript's time: `copy()` serialized the value in a managed module, a bug of perasc found here, F124, fixed before this run. What is left of A1's time is the copy itself: the notes' `add` copies each note's tags where TypeScript's spread shares them, the value semantics of [the guide](../guide/collections.md#an-element-is-a-value). A3's time is not explained yet; it is recorded as a question of the framebuffer's decision, whose program it is.

The Peras versions are 23 to 42 % longer than their TypeScript, for the reasons the ranking names: conversions, loops written where TypeScript has a method or a spread, a clamp and a compaction written by hand.

## Proposal (2026-11-25, for the owner)

The ranking, as registered, gives the order:

1. **The three causes that block a program**, tied: `Framebuffer<Rgba8>` (phase 6, A3), JSON in owned modules (F120, A4), a field of `inout self` given a whole value (F121, A3). Each is a decision of its own, measured; the framebuffer first, as it is also the next step of the roadmap the owner asked for, and A3's time is its question.
2. **A JavaScript number used as an integer and as a float**: 31 errors on four programs and three results changed in silence, the largest cause by far; a decision whose options are measured on these four programs.
3. Then the causes by their errors: the moves out of elements and fields (17), the syntax of TypeScript absent (14), the browser's names (14), an element read as a copy (10).

Packages and `@frame`, which the owner asked to build with the framebuffer as one lot, are not ranked by this measure, as registered: no program met them. They stay the owner's to order; this record says only that four applications written from TypeScript did not need them.

## The owner's decision (2026-11-25)

The ranking's order, with one change: the copies a later read never sees come before packages, as the largest cost of the applications measured (A1's 5.8 times TypeScript's time, a quarter of A3's, finding F125). The lot, in order: [decision 0047](0047-framebuffer.md), the framebuffer; then the copies, measured on A1 and A3; then packages and `create-peras`; then `@frame`; then an application that validates the lot, with the comparison against Rust and AssemblyScript measured again. JSON in owned modules (F120) and a field of `inout self` given a whole value (F121) follow the lot.
