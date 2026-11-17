# 0035: `Math` as JavaScript writes it, and rounding an integer

_French translation: [../../fr/decisions/0035-math-as-javascript-writes-it.md](../../fr/decisions/0035-math-as-javascript-writes-it.md)._

_Status: protocol registered on 2026-11-17, before any prototype and any measure, as the rule of the project asks; options measured at `6cb3444`; options a2 and b3 accepted by the owner on 2026-11-17. It answers findings F116 and F117, which [decision 0034](0034-integer-division.md) met, and what that decision left of F113: a percentage rounded, `Math.round(done * 100 / total)`, gives 66 % where JavaScript gives 67 %. Under the owner's rule of 2026-11-17, the option chosen must be an improvement, never a regression._

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

### A precision of the protocol (2026-11-17)

Point 3 measures R1 over 10,000,000 turns. R1 returns a string: its calls would measure the crossing to JavaScript and the strings it makes, not `Math`. Before any measure, R6, a float rounded, took its place, over 10,000,000 calls; R4 runs its own loop of 10,000,000 pixels, as registered.

### The measure (2026-11-17)

Run 1, at commit 6cb3444, nothing else running; every table in [`bench/math/RESULTS.md`](../../../bench/math/RESULTS.md), with the machine; non-regression by `node scripts/check-regression.mjs 6cb3444 PERAS_PROTO_MATH=a1` and so on, each option alone.

| Pair  | Programs right, of 6  | R5, the percentage (67 %) | Lines changed, R1 to R6 | W1, / a1 b1            | Bytes of W1 (gzip) |
| ----- | --------------------- | ------------------------- | ----------------------- | ---------------------- | ------------------ |
| a0 b0 | 0: all stuck at E0200 | stuck                     | -                       | -                      | -                  |
| a1 b1 | 5                     | 66 %                      | 4, 2, 2, 2, 2, 2        | 1                      | 135 (123)          |
| a1 b2 | 5                     | 66 %, silently            | 4, 2, 2, 2, 2, 2        | 1.001                  | 135 (123)          |
| a1 b3 | **6**                 | 67 %                      | 4, 2, 12, 4, 2, 2       | 0.846                  | 148 (140)          |
| a2 b1 | 5                     | 66 %                      | 0, 0, 2, 2, 2, 0        | 1.000                  | 135 (123)          |
| a2 b2 | 5                     | 66 %, silently            | 0, 0, 0, 0, 0, 0        | 1.001                  | 135 (123)          |
| a2 b3 | **6**                 | 67 %                      | 0, 0, 12, 4, 2, 0       | 0.845 (0.816 to 0.922) | 148 (140)          |

- **Non-regression**: each option alone, against 6cb3444: 167 modules of the repository compared, every one identical; none no longer builds; a2 builds R1, R2 and R6 of this decision, which nothing built before. Compile time from 0.973 to 1.010.
- **W2**, R6 over 10,000,000 calls: every pair within 0.999 to 1.032 of the first, the same code; the crossing to JavaScript is what it measures.
- **a2 takes the programs as JavaScript wrote them**: R1, R2 and R6 build unchanged and give JavaScript's result, `Math.round(12.5)` 13 included.
- **b2 gives a wrong number silently**: `Math.round(done * 100 / total)` builds and gives 66 %, the integer division never shown. b1 shows it no better: its fix removes the call, and the percentage stays 66 %.
- **b3 is the only one that gives every result right**, with a1 or a2. It costs lines where an integer was meant: `Math.floor((lo + hi) / 2)` becomes a float division floored, and the middle a float, converted back at each of its uses as an index, 12 lines in R3; in exchange, it floors as JavaScript does on negative numbers too, where truncation does not (`Math.floor(-7 / 2)` is -4; `-7 / 2` is -3). Its loop is the faster on W1, a float division costing less than a checked integer one.

### Proposal

**a2 with b3**: every program gives JavaScript's result, R1, R2 and R6 build as written, and no module of the repository changes. Its cost, the lines R3 asks to use a floored middle as an index, is the price of the right result on negative numbers; a later step could spare it where both operands are known not to be negative. The owner decides.
