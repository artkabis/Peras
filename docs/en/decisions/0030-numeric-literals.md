# 0030: Numbers written as JavaScript writes them

_French translation: [../../fr/decisions/0030-numeric-literals.md](../../fr/decisions/0030-numeric-literals.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks. It answers finding F112, the first of the corrections the owner ordered after [decision 0029](0029-pond-of-lanterns.md)._

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

## Record

_Each part is recorded here as it lands._
