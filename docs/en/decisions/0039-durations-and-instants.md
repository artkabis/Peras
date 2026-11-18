# 0039: Durations and instants

_French translation: [../../fr/decisions/0039-durations-and-instants.md](../../fr/decisions/0039-durations-and-instants.md)._

_Status: protocol registered on 2026-11-18, before any prototype and any measure, as the rule of the project asks. It answers the "time" line of phase 3 of the [roadmap](../reference/roadmap.md), durations and instants; dates of the calendar and time zones are left to a later decision. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

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
