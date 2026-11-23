# 0039: Durations and instants

_French translation: [../../fr/decisions/0039-durations-and-instants.md](../../fr/decisions/0039-durations-and-instants.md)._

_Status: protocol registered on 2026-11-18, before any prototype and any measure, as the rule of the project asks; options measured at `370b4ce`; t1 chosen by the owner on 2026-11-23. It answers the "time" line of phase 3 of the [roadmap](../reference/roadmap.md), durations and instants; dates of the calendar and time zones are left to a later decision. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

## Context

What Peras does today, read before writing this (finding M020):

- **Time is a number of milliseconds.** A managed module has `now(): f64`, the milliseconds since the page started, as `performance.now()`, and `setTimeout(ms, f)` and `setInterval(ms, f)`, which take a number ([browser reference](../reference/browser.md)).
- **Nothing tells a duration from an instant, or a millisecond from a second**: `setTimeout(5, f)` meant as five seconds builds and runs after five milliseconds; `now() + now()` builds.
- **An owned module has no time at all**: it cannot read a clock, and a duration in it is whatever number the program says it is.

## Options

- **t0. As today**: numbers of milliseconds.
- **t1. `Duration` and `Instant` types**, each a number underneath, at no cost: `Duration.ms(300)`, `Duration.seconds(5)`, `Duration.minutes(2)`, `d.toMs()`, `d.toSeconds()`; `Instant.now()` in a managed module; `b - a` of two instants is a `Duration`, an instant plus a duration an `Instant`, two durations add, a duration times a number is one; `setTimeout` and `setInterval` take a `Duration`, a number there an error whose fix writes the unit. In an owned module, durations without a clock.
- **t2. t1, with literals that carry their unit**: `300ms`, `5s`, `2min`, `1h` are `Duration`s, as `2.5` is a float.

## Protocol, registered before any prototype

Each option is prototyped behind `PERAS_PROTO_TIME`; the build without it keeps today's behaviour, t0. Every module measured is a production build, named (finding M029).

1. **Bugs caught**, as decision 0011 counts them: six programs, each with one bug of time a person writes, in TypeScript and in Peras under each option; whether the bug builds unseen, is refused with a diagnostic that says it, or cannot be written:
   - **B1**, five seconds written `setTimeout(5, …)`;
   - **B2**, seconds added to milliseconds, `start + 30` for thirty seconds;
   - **B3**, two instants added, `now() + start`;
   - **B4**, a duration read as seconds where it is milliseconds, `elapsed / 60` for minutes;
   - **B5**, a timeout of `minutes * 60 * 100` for milliseconds, a zero missing;
   - **B6**, an instant of `performance.now()` compared with one of `Date.now()`, two clocks.
2. **Ease of writing**, four programs written for each option, counted as decision 0028 counts them (lines, notions to learn, errors met):
   - **P1**, a search that waits 300 ms after the last key;
   - **P2**, the time a computation took, shown in milliseconds with one decimal;
   - **P3**, a countdown, "2 h 05 min" from a duration;
   - **P4**, in an owned module, the total length of a playlist, its tracks in seconds.
3. **Correctness**: each program gives TypeScript's result; an end-to-end test in both modes.
4. **Speed and load on the processor**, in Node, TypeScript and every option's module in one process, the order alternating, 21 rounds, the median of the ratios round by round and their range (finding M017), the CPU time over the wall time: **W1**, 1,000,000 durations added and compared, in a managed and in an owned module, against TypeScript's numbers: a duration must cost what a number costs.
5. **Bytes**: each module, raw and gzipped, and every module of the repository by `scripts/check-regression.mjs` under each option, which also gives the compile time and the compiler's size.
6. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-time.mjs` runs points 1, 2 and 4; `--check` checks every option's results against TypeScript's, and is run with every check of the project.

## Record

_Each part is recorded here as it lands._

### The measure (2026-11-23)

Two runs at commit 370b4ce, one after the other, nothing else running, every module built for production; every table in [`bench/time/RESULTS.md`](../../../bench/time/RESULTS.md), with the machine (an Intel Xeon at 2.10 GHz). Under t2 a bug is written as under t1: it is the unit that is missing.

| Bug                                                   | TypeScript    | t0, today     | t1 and t2                                               |
| ----------------------------------------------------- | ------------- | ------------- | ------------------------------------------------------- |
| B1, five seconds written `setTimeout(5, …)`           | builds unseen | builds unseen | refused, E0210: a number is not a `Duration`            |
| B2, `start + 30` for thirty seconds                   | builds unseen | builds unseen | refused, E0210                                          |
| B3, two instants added                                | builds unseen | builds unseen | refused, E0221: `+` cannot take `Instant` and `Instant` |
| B4, milliseconds read as seconds                      | builds unseen | builds unseen | refused, E0210, where the minutes meet `5`              |
| B5, a zero missing, `Duration.ms(minutes * 60 * 100)` | builds unseen | builds unseen | builds unseen                                           |
| B6, an instant of another clock                       | builds unseen | builds unseen | refused, E0210, but its fix is wrong (finding F225)     |

- **t1 and t2 catch four bugs of six**, TypeScript and today's Peras none. B5 is not caught: written in milliseconds, the number is a number; `Duration.minutes(minutes)` would not let the zero go missing, which no compiler can make a person write. B6 is refused, the other clock's number being no `Instant`, but the fix proposed, `Duration.ms(created)`, makes it a duration, which is wrong: it is an instant of another clock (finding F225).
- **Ease**: every program takes the same lines under the three options (P1 11, P2 13, P3 9, P4 11, W1 12), none met an error while written. What changes is what is learnt: t1 adds two types, four ways to make a duration, four to read one and `Instant.now()`; t2 adds the four suffixes, `ms`, `s`, `min` and `h`, and writes `300ms` where t1 writes `Duration.ms(300)`.
- **Speed, load and bytes**: a time costs a number. W1's modules under t1 and t2 are those of t0, byte for byte, in both modes (506 bytes in JavaScript, 170 in WebAssembly); W1 runs at 0.90 to 0.92 of TypeScript's time in a managed module and 0.63 to 0.64 in an owned one under every option, the CPU time over the wall time 0.9 to 1.0.
- **Non-regression**, `scripts/check-regression.mjs 0158806` under t1 and t2: no module larger, smaller or changed; compile time 1.012 to 1.024. But **three programs of the repository no longer build**, each waiting a number of milliseconds: the case study's `orders.apeiron` and `orders-extern.apeiron`, and the end-to-end fixture `browser/ui.apeiron`, with six examples of the documentation. Choosing t1 or t2 changes the language: every `setTimeout(300, …)` writes its unit. The other modules that no longer build are this decision's programs for t0.
- **An owned module** has durations and no clock: `Instant.now()` there is E0900, as the protocol said.

### Proposal

**t1**, `Duration` and `Instant`: four of the six bugs caught, at no cost in time or bytes, every program as long as today's. t2 catches the same bugs at the same cost and writes a duration in fewer characters, `300ms`, for four more notions and a change of the lexer; it can be added later on t1 without changing anything t1 writes, which t1 could not be after t2. Its price, in either case, is the migration of every timer that waits a bare number: three programs and six examples here, and each user's. The owner decides.

### Decision and landing (2026-11-23)

The owner chose **t1**. `Duration` and `Instant` are always there; `PERAS_PROTO_TIME` is gone, and with it t2's literals, their figures above and their code in git at 370b4ce. Every timer of the repository that waited a bare number now says its unit, `setTimeout(Duration.ms(300), …)`: the case study's two versions, the end-to-end fixture, two web bugs, six tests of the checker and of the generator, and the examples of the guide and of the browser's reference. A number taken from an `Instant` is now `check-time-either`, whose fix names both readings and edits nothing (finding F225). A test of the checker holds the four errors, one of the generator durations and instants computed in milliseconds. `scripts/bench-time.mjs` measures the build of today against TypeScript into `bench/time/CURRENT.md`; [`RESULTS.md`](../../../bench/time/RESULTS.md) keeps the options as measured.

Non-regression, `scripts/check-regression.mjs d964ca4`: no module larger, smaller or changed; compile time 0.976. The three programs migrated build into the JavaScript they built into before, byte for byte (checked against d964ca4's compiler on their old sources: a whole literal waited is written `50`, not `50.0`); the other modules that build in one compiler only are this decision's programs, and `b1-t1.apeiron`, bug B1, refused as it should be.
