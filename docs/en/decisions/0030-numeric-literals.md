# 0030: Numbers written as JavaScript writes them

_French translation: [../../fr/decisions/0030-numeric-literals.md](../../fr/decisions/0030-numeric-literals.md)._

_Status: protocol registered on 2026-11-17 (commit `f73b8f6`), before any prototype and any measure, as the rule of the project asks; options measured at `bb91877`; option b accepted by the owner on 2026-11-17. It answers finding F112, the first of the corrections the owner ordered after [decision 0029](0029-pond-of-lanterns.md)._

## Context

What the checker does today, read before writing this (finding M020): an integer literal has the type `{integer}`, which becomes any integer type its context asks for, and never a float (`E0211`, `crates/perasc-check/src/expr.rs`, `coerce`). The page of `E0211` gives the reason: "the decimal point makes the intent visible". An array literal takes the type of its first element, then refuses the others (`E0210`).

Decision 0029 measured what it costs: the pond, written from its TypeScript, met 45 errors before it built, 35 `E0211` (`2 * x`, `if (c < 0)`, `v = 1;` on floats) and 10 `E0210` (`[1, s, 0, -s]` with `s` a float, and the lines that followed). No program written for the project met more.

What the other languages do with `2 * x`, `x` a float: TypeScript has one type of number; Rust refuses it, as Peras does; Swift and Go accept it, a literal taking the type it is used as (Swift's `ExpressibleByIntegerLiteral`, Go's untyped constants); none of the four converts an integer _variable_ to a float silently, except JavaScript, where every number is one.

## Options

- **a. As today**: `E0211`, with the fix `2.0`.
- **b. A literal takes the type it is used as**, as in Swift and Go: an integer literal where a float is expected is that float, `2 * x`, `c < 0`, `v = 1`; an array literal of numbers takes the type of its elements together, a float if one is a float, `[1, s, 0]`. A variable is never converted: `n * x`, `n` an `i32`, stays refused, with the fix `n.toF64()`.
- **c. b, and an integer variable becomes a float where a float is expected**, as JavaScript does: `n * x` and `sum / xs.length` convert the integer. An `i64` beyond 2⁵³ loses digits without a word.

A fourth was considered and is not an option here: a variable declared from an integer literal, `let sum = 0;`, typed by a later use as a float, `sum += x`. Neither Swift nor Go does it, and the checker types a declaration when it reads it; it would be a decision on inference, not on literals.

## Protocol, registered before any prototype

Four quantities, as for decisions 0027 and 0028.

1. **Ease of writing**: five programs, written as a user writes them from JavaScript, then fixed until they build, for each option:
   - P1, the pond of decision 0029 as first written (`bench/lanterns/as-written/pond.peras`);
   - P2, the mean and the standard deviation of an `Array<f64>`, `sum / xs.length`;
   - P3, a step of particles: positions and speeds slowed, bounced off the walls at 0 and 1;
   - P4, prices in cents, an `i64`, with a tax of 20 % and a rounding;
   - P5, a colour mixed from three components and a gradient, `[0, 0.5, 1]`.

   For each: the errors met before it builds, with their codes, the lines changed from the JavaScript, the notions JavaScript does not ask for.

2. **What each option lets through**: for each program, a result that differs from what the same source gives as a managed module, which is JavaScript, counts as a silent change. P4 is written with a value beyond 2⁵³ for that.
3. **Speed, load on the processor, memory and bytes**: the pond as written, built by b and c, against the pond fixed, built by a: the bytes of each module, raw and gzip, equal or not; the time of frame 600 at 1024 × 768 in one process, the versions alternating, 15 rounds after 2, the ratio round by round (finding M017), and the CPU time over the wall time.
4. **The modes agree**: each fixed program gives, as an owned module on both allocators, what it gives as a managed module.
5. **An option not built** is reported as such, with why, never ranked by a guess (finding M001).

`node scripts/bench-numbers.mjs` runs them; `--check` checks that every program builds and agrees, and is run with every check of the project.

## Decision

**Option b: a literal takes the type it is used as**, as in Swift and Go.

1. An integer literal where a float is expected is that float: `2 * x`, `c < 0`, `v = 1;`, `return 0;`, an argument, `-1`. A composite of literals is computed in that float, `1 / 2` where an `f64` is expected being 0.5, as in Swift and in JavaScript.
2. An array literal of numbers takes the type of its elements together: a float if one of them is, `[1, s, 0]`.
3. A variable is never converted: `n * x`, `n` an `i32`, stays refused (`E0210`), with the fix `n.toF64()`. Where a value changes type, the source says so.
4. `E0211` is retired: its page stays, marked as such. Options a and c stay buildable behind `PERAS_PROTO_NUM`, for the measure script only.

## Criteria of the step, registered before its code

1. **UI tests**: a page of literals used as floats checks with no error, `f64` and `f32`, negative, in an operation, a comparison, an assignment, a return, an argument and an array literal; `n * x` with `n` an `i32` stays refused, with the fix `n.toF64()`; `E0211`'s test shows it builds.
2. **End to end, on both allocators**, against the same source built as a managed module: the same page computed, `1 / 2` where an `f64` is expected giving 0.5 in both modes, an `f32` literal rounded as an `f32`. The same results, nothing left alive.
3. **The pond as first written builds** and draws the pond's pixels: `node scripts/bench-numbers.mjs --check`; the scripts that rely on today's refusals, `bench-lanterns.mjs` for its kept version, ask for option a explicitly (finding M007).
4. **The rule broken on purpose is caught**: a literal typed as a float lowered as an integer constant, in the lowering, is refused by the release verifier or fails the end-to-end tests.
5. **Measures**, reported, not conditions: `node scripts/bench-numbers.mjs` at the commit of the step; the dashboard's A/B against the commit before (`ab-data.mjs`).
6. **Docs**, in English and in French: the guide's page on numbers, the types reference, `E0211`'s page, the status, finding F112, the changelog, `CLAUDE.md`.

## Record

_Each part is recorded here as it lands._

- **2026-11-17, the options measured** (`node scripts/bench-numbers.mjs`, [results](../../../bench/numbers/RESULTS.md); prototypes at `bb91877`, behind `PERAS_PROTO_NUM`). An Intel Xeon at 2.30 GHz, 4 cores, Node 24.
  - **Ease of writing**, the five programs and P5′:

    |       | Programs that build as written | Errors met | Lines changed | Results that differ from JavaScript as written |
    | ----- | -----------------------------: | ---------: | ------------: | ---------------------------------------------: |
    | a     |                         0 of 6 |         68 |           108 |                                              2 |
    | **b** |                         2 of 6 |          7 |            10 |                                              2 |
    | c     |                         6 of 6 |          0 |             0 |                                              2 |

    The pond as first written, 45 errors today, builds with b and with c. What b still refuses is an integer _variable_ used as a float: `sum / xs.length` (P2), `cents * 1.2` (P4), `t` of `i / (steps - 1)` (P5, P5′), each with the fix `.toF64()`.

  - **What each option lets through**: the same two results differ from JavaScript in every option, and neither is an option's doing. P4's first price, 9,007,199,254,740,993, is beyond what a JavaScript number holds: JavaScript loses its last digit, Peras's `i64` keeps it. P5′'s `i / (count - 1)` is an integer division in Peras, a fraction in JavaScript: 1 against 2.5, whichever option; a and b refuse `total += t` and their fix, `t.toF64()`, converts the integer already truncated (finding F113). Every program gives the same result owned on both allocators and managed.
  - **Speed, load and bytes**: the pond as written, built by b and by c, against the pond fixed, built by a, frame 600, 15 rounds: 1.033 (0.937 to 1.108) and 1.009 (0.886 to 1.095), the processor's load 1.00; the modules within 5 bytes of each other (14,647 and 14,652, 5,591 to 5,592 gzipped), the same pixels. No option changes the program it compiles; only what it accepts.
- **Departures from the protocol.**
  - **P5′ was added after registering**: P5's total does not depend on `t`, so it could not show an integer division.
  - **Results are compared with the programs written in JavaScript**, in `scripts/bench-numbers.mjs`, besides the managed module: a managed module follows the same rules as the owned one, so it cannot show what an option changes against JavaScript.
  - **P4 does not test c's loss of digits**: its tax applies to a price below 2⁵³; c converts an `i64` to a float without a mark in the source, which a measure of results cannot show, JavaScript losing the same digits.
- **What the measures say**: b removes 61 of the 68 errors (90 %) with no conversion the source does not show, at no cost in speed or bytes; c removes the last 7 by converting integer variables silently, the way JavaScript does and Swift, Go and Rust do not. Neither catches the integer division of P5′, a matter of its own (F113). b is the option to propose to the owner, who chooses.
