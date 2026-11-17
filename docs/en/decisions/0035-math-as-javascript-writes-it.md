# 0035: `Math` as JavaScript writes it, and rounding an integer

_French translation: [../../fr/decisions/0035-math-as-javascript-writes-it.md](../../fr/decisions/0035-math-as-javascript-writes-it.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks. It answers findings F116 and F117, which [decision 0034](0034-integer-division.md) met, and what that decision left of F113: a percentage rounded, `Math.round(done * 100 / total)`, gives 66 % where JavaScript gives 67 %. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

## Context

What the compiler does today, read before writing this (finding M020):

- **`Math` is not defined, in either mode**: `Math.floor(x)`, `Math.max(a, b)`, `Math.PI` are `` `Math` is not defined `` (E0200), whose fix, "declare it, or check its spelling", offers nothing (F116). Peras writes them as methods: `abs`, `min`, `max`, `clamp` on every number; `sqrt`, `sin`, `cos`, `tan`, `floor`, `ceil`, `round`, `exp`, `log`, `pow`, `atan2`, `hypot` on floats (`crates/perasc-check/src/builtins.rs`). It has no `PI`, no `E`, no `trunc`.
- **An integer has no `floor`, `round`, `ceil`**: `(p / width).floor()` is E0215, with no fix (F117). An integer is whole already, but an integer division is truncated: `round` on one is a fraction rounded the wrong way, `Math.round(200 / 3)` 66 where JavaScript gives 67.
- **Every diagnostic blocks the build**: there are no warnings.

## Options

Two questions, measured together, since one leads to the other: `Math.round(a / b)` written away becomes `(a / b).round()`.

- **Question A, `Math`:**
  - **a0.** As today.
  - **a1. The error offers the method**: `Math.f(x)` is E0200 with the fix `x.f()`, `Math.f(a, b)` with `a.f(b)`, `Math.PI` with its value written, `3.141592653589793`.
  - **a2. `Math` accepted as written**: `Math.f(x)` is `x.f()` in an owned module, JavaScript's `Math.f(x)` in a managed one; `Math.PI` and `Math.E` are those numbers.
- **Question B, `floor`, `round`, `ceil` or `trunc` on an integer:**
  - **b0.** As today.
  - **b1. The error offers the call removed**, the integer being whole already.
  - **b2. Accepted**: the call gives the integer.
  - **b3. Told apart**: on an integer division, the error shows the division and converts its operands, as decision 0034's d1, so that the fraction is rounded, `((done * 100).toF64() / total.toF64()).round()`; on any other integer, the fix removes the call.

## Protocol, registered before any prototype

Each option is prototyped behind `PERAS_PROTO_MATH` (a0 to a2) and `PERAS_PROTO_WHOLE` (b0 to b3); the build without them keeps today's behaviour, a0 and b0. The pairs measured: a0 b0, and each of a1 and a2 with each of b1, b2 and b3.

1. **Ease of writing, and the results the fixes give**: six programs, written as a user writes them from JavaScript, then fixed by applying the edits their diagnostics offer, as given, nothing else, until they build; a program whose diagnostics offer no edit is stuck:
   - R1, a distance and an angle, `Math.sqrt(dx * dx + dy * dy)`, `Math.atan2(dy, dx) * 180 / Math.PI`;
   - R2, a colour component clamped, `Math.min(Math.max(v, 0), 255)`, on integers;
   - R3, the middle of a binary search, `Math.floor((lo + hi) / 2)` (decision 0034's Q3);
   - R4, a pixel's row, `Math.floor(p / width)` (Q4);
   - R5, a percentage, `Math.round(done * 100 / total)` (Q5);
   - R6, a price rounded to cents, `Math.round(price * 100) / 100`, on a float.

   For each pair: the errors met, with their codes; the lines changed; whether the program builds; and whether it gives JavaScript's result. A wrong result reached by following the compiler's own fixes counts against the pair, whatever else it gains.

2. **Non-regression** (the owner's rule): `scripts/check-regression.mjs` under each option against today's compiler: programs that no longer build, or that change.
3. **Speed and load on the processor**: R4 and R1 as each pair fixed them, over 10,000,000 turns, owned modules on TLSF in Node, in one process, the order alternating, 21 rounds, the median of the ratios to the first pair that builds them, round by round, and their range (finding M017), the CPU time over the wall time. A pair that leaves a program stuck has no version of it: measured as not built (finding M001).
4. **Bytes**: each version's module, raw and gzipped.

`node scripts/bench-math.mjs` runs them; `--check` checks that every program builds and gives what it gave, and is run with every check of the project.

## Record

_Each part is recorded here as it lands._
